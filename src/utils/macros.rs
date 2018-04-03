macro_rules! get_arg {
    ( $args:ident, $extract_type:ident, $result_type:ty, $name:ident ) => (
        match $args.$extract_type::<$result_type>() {
            Ok(x)  => x,
            Err(_) => {
                return Err(CommandError::from(format!("Error parsing argument: {}!", stringify!($name))));
            },
        }
    );

    ( $args:ident, $extract_type:ident, $result_type:ty, $name:ident, $default:expr ) => (
        match $args.$extract_type::<$result_type>() {
            Ok(x)  => x,
            Err(_) => $default,
        }
    );
}


macro_rules! extract_pool {
    ( $ctx:expr ) => (
        &*$ctx.data.lock().get::<PgConnectionManager>().unwrap().get().unwrap()
    )
}
