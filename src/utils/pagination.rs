use diesel::pg::Pg;
use diesel::prelude::*;
use diesel::query_builder::*;
use diesel::query_dsl::methods::LoadQuery;
use diesel::sql_types::BigInt;
use itertools::Itertools;


#[derive(Debug, Clone, Copy, QueryId)]
pub struct Paginated<T> {
    query: T,
    page: i64,
    per_page: i64,
}


#[derive(Debug, Clone)]
pub struct PaginationResult<T> {
    pub results: Vec<T>,
    pub total: i64,
    pub total_pages: i64,
    pub page: i64,
    pub per_page: i64,
}

// pagination stuff copied from
// https://github.com/diesel-rs/diesel/examples/postgres/advanced-blog-cli/src/pagination.rs

const DEFAULT_PER_PAGE: i64 = 12;

impl<T> PaginationResult<T> {
    pub fn iter_with_indexes(&self) -> impl Iterator<Item = (&T, usize)> {
        let offset = ((self.page - 1) * self.per_page + 1) as usize;

        self.results
            .iter()
            .zip(offset..)
    }

    /// Format paginated result into lines
    pub fn lines(&self, mut line_formatter: impl FnMut(&T, usize) -> String) -> String {
        self.iter_with_indexes()
            .map(|(t, i)| line_formatter(t, i))
            .join("\n")
    }

    pub fn block(&self, lineformatter: impl FnMut(&T, usize) -> String) -> String {
        let lines = self.lines(lineformatter);
        let lines = super::normalize(&lines).replace("```", "   ");

        format!("```\n{}\n```\nPage {} of {}", lines, self.page, self.total_pages)
    }

    pub fn page_exists(&self) -> bool {
        self.page <= self.total_pages
    }
}


impl<T> Paginated<T> {
    pub fn per_page(self, per_page: i64) -> Self {
        Paginated { per_page, ..self }
    }

    pub fn load_and_count_pages<U>(self, conn: &PgConnection) -> QueryResult<PaginationResult<U>>
    where
        Self: LoadQuery<PgConnection, (U, i64)>,
    {
        let per_page = self.per_page;
        let page = self.page;

        let results = self.load::<(U, i64)>(conn)?;

        let total = results.get(0).map(|x| x.1).unwrap_or(0);
        let records = results.into_iter().map(|x| x.0).collect();

        let total_pages = (total + per_page - 1) / per_page;
        Ok(PaginationResult {
            results: records,
            total,
            total_pages,
            page: page,
            per_page: per_page,
        })
    }
}

impl<T: Query> Query for Paginated<T>  {
    type SqlType = (T::SqlType, BigInt);
}

impl<T> RunQueryDsl<PgConnection> for Paginated<T> {}

impl<T> QueryFragment<Pg> for Paginated<T>
where
    T: QueryFragment<Pg>,
{
    fn walk_ast(&self, mut out: AstPass<Pg>) -> QueryResult<()> {
        out.push_sql("SELECT *, COUNT(*) OVER() FROM (");
        self.query.walk_ast(out.reborrow())?;
        out.push_sql(") t LIMIT ");
        out.push_bind_param::<BigInt, _>(&self.per_page)?;
        out.push_sql(" OFFSET ");
        let offset = (self.page - 1) * self.per_page;
        out.push_bind_param::<BigInt, _>(&offset)?;
        Ok(())
    }
}

pub trait Paginate: Sized {
    fn paginate(self, page: i64) -> Paginated<Self>;
}

impl<T> Paginate for T {
    fn paginate(self, page: i64) -> Paginated<Self> {
        Paginated {
            query: self,
            page,
            per_page: DEFAULT_PER_PAGE,
        }
    }
}
