use std::cell::RefCell;
use std::collections::hash_map::Entry;
use std::collections::HashMap;
use std::fmt::Write;
use std::iter::Iterator;
use std::marker::PhantomData;
use std::rc::Rc;

use scoped_tls::scoped_thread_local;
use zvariant::{ObjectPath, OwnedValue, Value};

use crate as zbus;
use crate::{dbus_interface, fdo, Connection, Error, Message, MessageHeader, MessageType, Result};

scoped_thread_local!(static LOCAL_NODE: ObjectNode);

/// The trait used to dispatch messages to an interface instance.
///
/// Note: It is not recommended to manually implement this trait. The [`dbus_interface`] macro
/// implements it for you.
///
/// [`dbus_interface`]: attr.dbus_interface.html
pub trait Interface {
    /// Return the name of the interface. Ex: "org.foo.MyInterface"
    fn name() -> &'static str
    where
        Self: Sized;

    /// Get a property value. Returns `None` if the property doesn't exist.
    fn get(&self, property_name: &str) -> Option<fdo::Result<OwnedValue>>;

    /// Return all the properties.
    fn get_all(&self) -> HashMap<String, OwnedValue>;

    /// Set a property value. Returns `None` if the property doesn't exist.
    fn set(&mut self, property_name: &str, value: &Value) -> Option<fdo::Result<()>>;

    /// Call a `&self` method. Returns `None` if the method doesn't exist.
    fn call(&self, connection: &Connection, msg: &Message, name: &str) -> Option<Result<u32>>;

    /// Call a `&mut self` method. Returns `None` if the method doesn't exist.
    fn call_mut(
        &mut self,
        connection: &Connection,
        msg: &Message,
        name: &str,
    ) -> Option<Result<u32>>;

    /// Write introspection XML to the writer, with the given indentation level.
    fn introspect_to_writer(&self, writer: &mut dyn Write, level: usize);
}

struct Introspectable;

#[dbus_interface(name = "org.freedesktop.DBus.Introspectable")]
impl Introspectable {
    fn introspect(&self) -> String {
        LOCAL_NODE.with(|node| node.introspect())
    }
}

struct Peer;

#[dbus_interface(name = "org.freedesktop.DBus.Peer")]
impl Peer {
    fn ping(&self) {}

    fn get_machine_id(&self) -> fdo::Result<String> {
        let mut id = match std::fs::read_to_string("/var/lib/dbus/machine-id") {
            Ok(id) => id,
            Err(e) => {
                if let Ok(id) = std::fs::read_to_string("/etc/machine-id") {
                    id
                } else {
                    return Err(fdo::Error::IOError(format!(
                        "Failed to read from /var/lib/dbus/machine-id or /etc/machine-id: {}",
                        e
                    )));
                }
            }
        };

        let len = id.trim_end().len();
        id.truncate(len);
        Ok(id)
    }
}

struct Properties;

#[dbus_interface(name = "org.freedesktop.DBus.Properties")]
impl Properties {
    fn get(&self, interface_name: &str, property_name: &str) -> fdo::Result<OwnedValue> {
        LOCAL_NODE.with(|node| {
            let iface = node.get_interface(interface_name).ok_or_else(|| {
                fdo::Error::UnknownInterface(format!("Unknown interface '{}'", interface_name))
            })?;

            let res = iface.borrow().get(property_name);
            res.ok_or_else(|| {
                fdo::Error::UnknownProperty(format!("Unknown property '{}'", property_name))
            })?
        })
    }

    // TODO: should be able to take a &Value instead (but obscure deserialize error for now..)
    fn set(
        &mut self,
        interface_name: &str,
        property_name: &str,
        value: OwnedValue,
    ) -> fdo::Result<()> {
        LOCAL_NODE.with(|node| {
            let iface = node.get_interface(interface_name).ok_or_else(|| {
                fdo::Error::UnknownInterface(format!("Unknown interface '{}'", interface_name))
            })?;

            let res = iface.borrow_mut().set(property_name, &value);
            res.ok_or_else(|| {
                fdo::Error::UnknownProperty(format!("Unknown property '{}'", property_name))
            })?
        })
    }

    fn get_all(&self, interface_name: &str) -> fdo::Result<HashMap<String, OwnedValue>> {
        LOCAL_NODE.with(|node| {
            let iface = node.get_interface(interface_name).ok_or_else(|| {
                fdo::Error::UnknownInterface(format!("Unknown interface '{}'", interface_name))
            })?;

            let res = iface.borrow().get_all();
            Ok(res)
        })
    }

    #[dbus_interface(signal)]
    fn properties_changed(
        &self,
        interface_name: &str,
        changed_properties: &HashMap<&str, &Value>,
        invalidated_properties: &[&str],
    ) -> Result<()>;
}

/// A D-Bus object node that is served by an [`ObjectServer`].
///
/// Each object node has a unique (across an ObjectServer instance) path and set of interfaces that
/// it implements. Instances of this type are created and managed by [`ObjectServer`] for you, and as
/// such no API is provided to instantiate it.
///
/// [`ObjectServer`]: struct.ObjectServer.html
#[derive(derivative::Derivative)]
#[derivative(Debug)]
pub struct ObjectNode {
    path: ObjectPath<'static>,
    children: HashMap<String, ObjectNode>,
    #[derivative(Debug = "ignore")]
    interfaces: HashMap<&'static str, Rc<RefCell<dyn Interface>>>,
    conn: Connection,
}

impl ObjectNode {
    /// The unique path of `self`.
    pub fn path(&self) -> &ObjectPath {
        &self.path
    }

    /// Check if `self` is providing the interface `iface`.
    pub fn has_interface(&self, iface: &str) -> bool {
        self.interfaces.contains_key(iface)
    }

    /// Get an iterator over all interfaces provided by `self`.
    pub fn interfaces(&self) -> impl Iterator<Item = &'static str> + '_ {
        self.interfaces.keys().copied()
    }

    /// The associated D-Bus connection.
    pub fn connection(&self) -> &Connection {
        &self.conn
    }

    /// Emit a signal from `self`.
    ///
    /// This is simply a wrapper around the [low-level signal emission API] that is slighly more
    /// convenient to use since you don't need to provide the object path or connection.
    ///
    /// You should only need to use this method when emitting a signal from outside of interface
    /// method handler context. From inside a method handler context, it's easier to make use of the
    /// derived signal implementation from [`dbus_interface`].
    ///
    /// [low-level signal emission API]: struct.Connection.html#method.emit_signal
    /// [`dbus_interface`]: attr.dbus_interface.html
    pub fn emit_signal<B>(
        &self,
        dest: Option<&str>,
        iface: &str,
        signal_name: &str,
        body: &B,
    ) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::Type,
    {
        self.conn
            .emit_signal(dest, &self.path, iface, signal_name, body)
    }

    fn new(path: &ObjectPath, connection: &Connection) -> Self {
        let mut node = Self {
            path: path.to_owned(),
            children: HashMap::new(),
            interfaces: HashMap::new(),
            conn: connection.clone(),
        };
        node.at(Peer::name(), Peer);
        node.at(Introspectable::name(), Introspectable);
        node.at(Properties::name(), Properties);

        node
    }

    fn get_interface(&self, iface: &str) -> Option<Rc<RefCell<dyn Interface + 'static>>> {
        self.interfaces.get(iface).cloned()
    }

    fn at<I>(&mut self, name: &'static str, iface: I) -> bool
    where
        I: Interface + 'static,
    {
        match self.interfaces.entry(name) {
            Entry::Vacant(e) => e.insert(Rc::new(RefCell::new(iface))),
            Entry::Occupied(_) => return false,
        };

        true
    }

    fn introspect_to_writer<W: Write>(&self, writer: &mut W, level: usize) {
        if level == 0 {
            writeln!(
                writer,
                r#"
<!DOCTYPE node PUBLIC "-//freedesktop//DTD D-BUS Object Introspection 1.0//EN"
 "http://www.freedesktop.org/standards/dbus/1.0/introspect.dtd">
<node>"#
            )
            .unwrap();
        }

        for iface in self.interfaces.values() {
            iface.borrow().introspect_to_writer(writer, level + 2);
        }

        for (path, node) in &self.children {
            let level = level + 2;
            writeln!(
                writer,
                "{:indent$}<node name=\"{}\">",
                "",
                path,
                indent = level
            )
            .unwrap();
            node.introspect_to_writer(writer, level);
            writeln!(writer, "{:indent$}</node>", "", indent = level).unwrap();
        }

        if level == 0 {
            writeln!(writer, "</node>").unwrap();
        }
    }

    fn introspect(&self) -> String {
        let mut xml = String::with_capacity(1024);

        self.introspect_to_writer(&mut xml, 0);

        xml
    }
}

/// An object server, holding server-side D-Bus objects & interfaces.
///
/// Object servers hold interfaces on various object paths, and expose them over D-Bus.
///
/// All object paths will have the standard interfaces implemented on your behalf, such as
/// `org.freedesktop.DBus.Introspectable` or `org.freedesktop.DBus.Properties`.
///
/// **NOTE:** The lifetime `'a` on the `ObjectServer` struct is bogus and only exists for
/// backwards-compatibility and will be dropped in the next major release (i-e 2.0). This means
/// that `'a` can be considered `'static` for all intents and purposes.
///
/// # Example
///
/// This example exposes the `org.myiface.Example.Quit` method on the `/org/zbus/path`
/// path.
///
/// ```no_run
///# use std::error::Error;
///# use std::convert::TryInto;
/// use zbus::{Connection, ObjectServer, dbus_interface};
/// use std::rc::Rc;
/// use std::cell::RefCell;
///
/// struct Example {
///     // Interfaces are owned by the ObjectServer. They can have
///     // `&mut self` methods.
///     //
///     // If you need a shared state, you can use a RefCell for ex:
///     quit: Rc<RefCell<bool>>,
/// }
///
/// impl Example {
///     fn new(quit: Rc<RefCell<bool>>) -> Self {
///         Self { quit }
///     }
/// }
///
/// #[dbus_interface(name = "org.myiface.Example")]
/// impl Example {
///     // This will be the "Quit" D-Bus method.
///     fn quit(&self) {
///         *self.quit.borrow_mut() = true;
///     }
///
///     // See `dbus_interface` documentation to learn
///     // how to expose properties & signals as well.
/// }
///
/// let connection = Connection::new_session()?;
/// let mut object_server = ObjectServer::new(&connection);
/// let quit = Rc::new(RefCell::new(false));
///
/// let interface = Example::new(quit.clone());
/// object_server.at(&"/org/zbus/path".try_into()?, interface)?;
///
/// loop {
///     if let Err(err) = object_server.try_handle_next() {
///         eprintln!("{}", err);
///     }
///
///     if *quit.borrow() {
///         break;
///     }
/// }
///# Ok::<_, Box<dyn Error + Send + Sync>>(())
/// ```
#[derive(Debug)]
pub struct ObjectServer<'a> {
    conn: Connection,
    root: ObjectNode,
    phantom: PhantomData<&'a ()>,
}

impl<'a> ObjectServer<'a> {
    /// Creates a new D-Bus `ObjectServer` for a given connection.
    pub fn new(connection: &Connection) -> Self {
        Self {
            conn: connection.clone(),
            root: ObjectNode::new(&ObjectPath::from_str_unchecked("/"), connection),
            phantom: PhantomData,
        }
    }

    // Get the ObjectNode at path, creating the node (hierarchy) if it doesn't exist.
    fn create_node(&mut self, path: &ObjectPath) -> &mut ObjectNode {
        let mut node = &mut self.root;
        let mut node_path = String::new();

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }
            write!(&mut node_path, "/{}", i).unwrap();
            match node.children.entry(i.into()) {
                Entry::Vacant(e) => {
                    let path = ObjectPath::from_str_unchecked(&node_path);

                    node = e.insert(ObjectNode::new(&path, &self.conn));
                }
                Entry::Occupied(e) => node = e.into_mut(),
            }
        }

        node
    }

    /// Get the object node registered at the specified path.
    ///
    /// Returns `None` if there is no registered object at specified path.
    pub fn get_object_node(&self, path: &ObjectPath) -> Option<&ObjectNode> {
        let mut node = &self.root;
        let mut node_path = String::new();

        for i in path.split('/').skip(1) {
            if i.is_empty() {
                continue;
            }
            write!(&mut node_path, "/{}", i).unwrap();
            match node.children.get(i) {
                None => return None,
                Some(n) => node = n,
            }
        }

        Some(node)
    }

    /// Register a D-Bus [`Interface`] at a given path. (see the example above)
    ///
    /// If the interface already exists at this path, returns false.
    ///
    /// [`Interface`]: trait.Interface.html
    pub fn at<I>(&mut self, path: &ObjectPath, iface: I) -> Result<bool>
    where
        I: Interface + 'static,
    {
        Ok(self.create_node(path).at(I::name(), iface))
    }

    /// Emit a signal on the currently dispatched object node.
    ///
    /// This is an internal helper function to emit a signal on the current object node. You
    /// shouldn't call this method directly, rather with the derived signal implementation from
    /// [`dbus_interface`].
    ///
    /// [`dbus_interface`]: attr.dbus_interface.html
    pub fn local_node_emit_signal<B>(
        destination: Option<&str>,
        iface: &str,
        signal_name: &str,
        body: &B,
    ) -> Result<()>
    where
        B: serde::ser::Serialize + zvariant::Type,
    {
        if !LOCAL_NODE.is_set() {
            return Err(Error::NoTLSNode);
        }

        LOCAL_NODE.with(|n| n.emit_signal(destination, iface, signal_name, body))
    }

    fn dispatch_method_call_try(
        &mut self,
        msg_header: &MessageHeader,
        msg: &Message,
    ) -> fdo::Result<Result<u32>> {
        let conn = self.conn.clone();
        let path = msg_header
            .path()
            .ok()
            .flatten()
            .ok_or_else(|| fdo::Error::Failed("Missing object path".into()))?;
        let iface = msg_header
            .interface()
            .ok()
            .flatten()
            // TODO: In the absence of an INTERFACE field, if two or more interfaces on the same object
            // have a method with the same name, it is undefined which of those methods will be
            // invoked. Implementations may choose to either return an error, or deliver the message
            // as though it had an arbitrary one of those interfaces.
            .ok_or_else(|| fdo::Error::Failed("Missing interface".into()))?;
        let member = msg_header
            .member()
            .ok()
            .flatten()
            .ok_or_else(|| fdo::Error::Failed("Missing member".into()))?;

        let node = self
            .get_object_node(&path)
            .ok_or_else(|| fdo::Error::UnknownObject(format!("Unknown object '{}'", path)))?;
        let iface = node.get_interface(iface).ok_or_else(|| {
            fdo::Error::UnknownInterface(format!("Unknown interface '{}'", iface))
        })?;

        LOCAL_NODE.set(node, || {
            let res = iface.borrow().call(&conn, &msg, member);
            res.or_else(|| iface.borrow_mut().call_mut(&conn, &msg, member))
                .ok_or_else(|| fdo::Error::UnknownMethod(format!("Unknown method '{}'", member)))
        })
    }

    fn dispatch_method_call(&mut self, msg_header: &MessageHeader, msg: &Message) -> Result<u32> {
        match self.dispatch_method_call_try(msg_header, msg) {
            Err(e) => e.reply(&self.conn, msg),
            Ok(r) => r,
        }
    }

    /// Dispatch an incoming message to a registered interface.
    ///
    /// The object server will handle the message by:
    ///
    /// - looking up the called object path & interface,
    ///
    /// - calling the associated method if one exists,
    ///
    /// - returning a message (responding to the caller with either a return or error message) to
    ///   the caller through the associated server connection.
    ///
    /// Returns an error if the message is malformed, true if it's handled, false otherwise.
    ///
    /// # Note
    ///
    /// This API is subject to change, or becoming internal-only once zbus provides a general
    /// mechanism to dispatch messages.
    pub fn dispatch_message(&mut self, msg: &Message) -> Result<bool> {
        let msg_header = msg.header()?;

        match msg_header.message_type()? {
            MessageType::MethodCall => {
                self.dispatch_method_call(&msg_header, &msg)?;
                Ok(true)
            }
            _ => Ok(false),
        }
    }

    /// Receive and handle the next message from the associated connection.
    ///
    /// This function will read the incoming message from
    /// [`receive_message()`](Connection::receive_message) of the associated connection and pass it
    /// to [`dispatch_message()`](Self::dispatch_message). If the message was handled by an an
    /// interface, it returns `Ok(None)`. If not, it returns the received message.
    ///
    /// Returns an error if the message is malformed or an error occured.
    ///
    /// # Note
    ///
    /// This API is subject to change, or becoming internal-only once zbus provides a general
    /// mechanism to dispatch messages.
    pub fn try_handle_next(&mut self) -> Result<Option<Message>> {
        let msg = self.conn.receive_message()?;

        if !self.dispatch_message(&msg)? {
            return Ok(Some(msg));
        }

        Ok(None)
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;
    use std::convert::{TryFrom, TryInto};
    use std::error::Error;
    use std::rc::Rc;
    use std::sync::{Arc, Condvar, Mutex};
    use std::thread;

    use crate as zbus;
    use crate::fdo;
    use crate::{dbus_interface, dbus_proxy, Connection, ObjectServer};
    use crate::{MessageField, MessageFieldCode, MessageType};
    use zvariant::{ObjectPath, Str};

    #[dbus_proxy]
    trait MyIface {
        fn ping(&self) -> zbus::Result<u32>;

        fn quit(&self, val: bool) -> zbus::Result<()>;

        fn test_error(&self) -> zbus::Result<()>;

        #[dbus_proxy(property)]
        fn count(&self) -> fdo::Result<u32>;

        #[dbus_proxy(property)]
        fn set_count(&self, count: u32) -> fdo::Result<()>;
    }

    #[derive(Debug)]
    struct MyIfaceImpl {
        quit: Rc<RefCell<bool>>,
        count: u32,
    }

    impl MyIfaceImpl {
        fn new(quit: Rc<RefCell<bool>>) -> Self {
            Self { quit, count: 0 }
        }
    }

    #[dbus_interface(interface = "org.freedesktop.MyIface")]
    impl MyIfaceImpl {
        fn ping(&mut self) -> u32 {
            self.count += 1;
            if self.count % 3 == 0 {
                self.alert_count(self.count).expect("Failed to emit signal");
            }
            self.count
        }

        fn quit(&mut self, val: bool) {
            *self.quit.borrow_mut() = val;
        }

        fn test_error(&self) -> zbus::fdo::Result<()> {
            Err(zbus::fdo::Error::Failed("error raised".to_string()))
        }

        #[dbus_interface(property)]
        fn set_count(&mut self, val: u32) -> zbus::fdo::Result<()> {
            if val == 42 {
                return Err(zbus::fdo::Error::InvalidArgs("Tsss tsss!".to_string()));
            }
            self.count = val;
            Ok(())
        }

        #[dbus_interface(property)]
        fn count(&self) -> u32 {
            self.count
        }

        #[dbus_interface(signal)]
        fn alert_count(&self, val: u32) -> zbus::Result<()>;
    }

    fn my_iface_test() -> std::result::Result<u32, Box<dyn Error>> {
        let conn = Connection::new_session()?;
        let proxy = MyIfaceProxy::new_for(
            &conn,
            "org.freedesktop.MyService",
            "/org/freedesktop/MyService",
        )?;

        proxy.ping()?;
        assert_eq!(proxy.count()?, 1);
        proxy.introspect()?;
        let val = proxy.ping()?;
        proxy.quit(true)?;
        Ok(val)
    }

    #[test]
    fn basic_iface() -> std::result::Result<(), Box<dyn Error>> {
        let conn = Connection::new_session()?;
        let mut object_server = ObjectServer::new(&conn);
        let quit = Rc::new(RefCell::new(false));

        fdo::DBusProxy::new(&conn)?.request_name(
            "org.freedesktop.MyService",
            fdo::RequestNameFlags::ReplaceExisting.into(),
        )?;

        let iface = MyIfaceImpl::new(quit.clone());
        object_server.at(&"/org/freedesktop/MyService".try_into()?, iface)?;

        let child = thread::spawn(|| my_iface_test().expect("child failed"));

        loop {
            let m = conn.receive_message()?;
            if let Err(e) = object_server.dispatch_message(&m) {
                eprintln!("{}", e);
            }

            if *quit.borrow() {
                break;
            }
        }

        let val = child.join().expect("failed to join");
        assert_eq!(val, 2);
        Ok(())
    }

    #[test]
    fn signal_emission() -> std::result::Result<(), Box<dyn Error>> {
        let conn = Connection::new_session()?;
        let mut object_server = ObjectServer::new(&conn);

        fdo::DBusProxy::new(&conn)?.request_name(
            "org.freedesktop.MyService2",
            fdo::RequestNameFlags::ReplaceExisting.into(),
        )?;

        let iface = MyIfaceImpl::new(Rc::new(RefCell::new(false)));
        object_server.at(&"/org/freedesktop/MyService".try_into()?, iface)?;

        let pair = Arc::new((Mutex::new(false), Condvar::new()));
        let pair2 = pair.clone();

        let child = thread::spawn(move || receive_signal(pair2).expect("child failed"));

        let path = ObjectPath::try_from("/org/freedesktop/MyService").unwrap();
        let object = object_server.get_object_node(&path).unwrap();

        // Wait for client thread to be ready
        let (lock, cvar) = &*pair;
        let result = cvar
            .wait_while(lock.lock().unwrap(), |&mut ready| !ready)
            .unwrap();
        assert!(*result);

        object
            .emit_signal(None, "org.freedesktop.MyService", "AlertCount", &42u32)
            .unwrap();

        let val = child.join().expect("failed to join");
        assert_eq!(val, 42);
        Ok(())
    }

    fn receive_signal(
        pair: Arc<(Mutex<bool>, Condvar)>,
    ) -> std::result::Result<u32, Box<dyn Error>> {
        let conn = Connection::new_session()?;

        // FIXME: Hopefully this get easier when we've high-level signal receiving API:
        // https://gitlab.freedesktop.org/zeenix/zbus/-/issues/46

        fdo::DBusProxy::new(&conn)?.add_match(
            "type='signal',sender='org.freedesktop.MyService2',interface='org.freedesktop.MyService',member='AlertCount'",
        )?;

        {
            let (lock, cvar) = &*pair;
            let mut ready = lock.lock().unwrap();
            *ready = true;
            cvar.notify_one();
        }

        loop {
            let m = conn.receive_message()?;

            let hdr = m.header().unwrap();
            let fields = m.fields().unwrap();
            let member = fields.get_field(MessageFieldCode::Member).unwrap();

            if hdr.primary().msg_type() == MessageType::Signal {
                if *member == MessageField::Member(Str::from("AlertCount")) {
                    return Ok(m.body()?);
                }
            }
        }
    }
}
