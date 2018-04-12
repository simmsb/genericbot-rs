use serenity::model::id::{GuildId, UserId};

#[macro_use]
pub mod macros;
pub mod markov;


pub fn names_for_members<U, G>(u_ids: &[U], g_id: G) -> Vec<String>
    where U: Into<UserId> + Copy,
          G: Into<GuildId> + Copy,
{
    use serenity::{
        utils::with_cache,
    };

    fn backup_getter<U>(u_id: U) -> String
        where U: Into<UserId> + Copy,
    {
        match u_id.into().get() {
            Ok(u) => u.name,
            _     => u_id.into().to_string(),
        }
    }

    with_cache(
        |cache| cache.guild(g_id).map(|g| {
            let members = &g.read().members;
            u_ids.iter().map(
                |&id| members.get(&id.into()).map_or_else(
                    || backup_getter(id),
                    |m| m.display_name().to_string()))
                           .collect()
        })).unwrap_or_else(|| u_ids.iter().map(|&id| backup_getter(id)).collect())
}


pub fn and_comma_split<T: AsRef<str>>(m: &[T]) -> String {
    let mut res = String::new();
    let end = m.len() as isize;

    for (n, s) in m.into_iter().enumerate() {
        res.push_str(s.as_ref());
        if n as isize == end - 2 {
            res.push_str(" and ");
        } else if (n as isize) < end - 2 {
            res.push_str(", ");
        }
    }
    return res;
}
