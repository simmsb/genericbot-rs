/// Some convenience macro for getting arguments
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


/// Macro for discarding the result of a `#[must_use]` function
macro_rules! void {
    ( $d:expr ) => (
        { if let Err(e) = $d {
            error!(target: "bot", "Got error {} from {}.", e, stringify!($d));
        } }
    )
}


/// Continues on a loop if a `Option` is not `Some`
///
/// `try_opt_continue!(X)` is equivalent to:
///
/// ```rust,ignore
/// match X {
///     Some(x) => x,
///     _       => continue,
/// }
/// ```
macro_rules! try_opt_continue {
    ( $e:expr ) => (
        match $e {
            Some(x) => x,
            _       => continue,
        }
    )
}


/// Continues on a loop if a `Result` is not an `Ok`
///
/// `try_continue!(X)` is equivalent to:
///
/// ```rust,ignore
/// match X {
///     Ok(x) => x,
///     _     => continue,
/// }
/// ```
macro_rules! try_continue {
    ( $e:expr ) => (
        match $e {
            Ok(x) => x,
            _     => continue,
        }
    )
}


macro_rules! log_time {
    ( $exp:expr, $name:expr ) => ( {
        use std::time::Instant;

        let now = Instant::now();
        let result = $exp;
        let duration = now.elapsed();

        debug!(target: "bot", "{} on line {} in file {} took {:?} to complete", $name, line!(), file!(), duration);
        result
    } )
}
