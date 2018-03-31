use serenity::prelude::*;
use serenity::framework::standard::StandardFramework;
use diesel::pg::PgConnection;
use std::sync::Arc;

use serenity::model::channel::Message;
use serenity::model::id::UserId;
use serenity::framework::{Framework};
use threadpool::ThreadPool;

struct MyFramework {
    inner: StandardFramework,
    pg_conn: Arc<Mutex<PgConnection>>,
}


impl Framework for MyFramework {

    fn dispatch(&mut self, ctx: Context, msg: Message, threadpool: &ThreadPool) {
        self.inner.dispatch(ctx, msg, threadpool);
    }

    fn update_current_user(&mut self, id: UserId) {
        self.inner.update_current_user(id);
    }
}
