use crate::{error::Result, User};
use futures::StreamExt;
use pointercrate_core::{
    first_and_last,
    pagination::{PageContext, Pagination, PaginationParameters, __pagination_compat},
    permission::Permission,
    util::{non_nullable, nullable},
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgRow, PgConnection, Row};

#[derive(Deserialize, Debug, Clone, Serialize)]
pub struct UserPagination {
    #[serde(flatten)]
    pub params: PaginationParameters,

    #[serde(default, deserialize_with = "non_nullable")]
    pub name: Option<String>,

    #[serde(default, deserialize_with = "non_nullable")]
    pub name_contains: Option<String>,

    #[serde(default, deserialize_with = "nullable")]
    pub display_name: Option<Option<String>>,

    #[serde(default, deserialize_with = "non_nullable")]
    pub has_permissions: Option<u16>,

    #[serde(default, deserialize_with = "non_nullable")]
    pub any_permissions: Option<u16>,
}

impl Pagination for UserPagination {
    type Item = User;

    fn parameters(&self) -> PaginationParameters {
        self.params
    }

    fn with_parameters(&self, parameters: PaginationParameters) -> Self {
        Self {
            params: parameters,
            ..self.clone()
        }
    }

    first_and_last!("members", "member_id");

    async fn page(&self, connection: &mut PgConnection) -> std::result::Result<(Vec<User>, PageContext), sqlx::Error> {
        let order = self.params.order();

        let query = format!(include_str!("../sql/paginate_users.sql"), order);

        let mut stream = sqlx::query(&query)
            .bind(self.params.before)
            .bind(self.params.after)
            .bind(self.name.as_ref())
            .bind(self.display_name.as_ref())
            .bind(self.display_name == Some(None))
            .bind(self.has_permissions.map(|p| p as i32))
            .bind(self.any_permissions.map(|p| p as i32))
            .bind(self.name_contains.as_ref())
            .bind(self.params.limit + 1)
            .fetch(connection);

        let mut users = Vec::new();

        while let Some(row) = stream.next().await {
            let row: PgRow = row?;

            let perms_as_i32: i32 = row.get("permissions");

            users.push(User {
                id: row.get("member_id"),
                name: row.get("name"),
                permissions: perms_as_i32 as u16,
                display_name: row.get("display_name"),
                youtube_channel: row.get("youtube_channel"),
            })
        }

        Ok(__pagination_compat(&self.params, users))
    }

    fn id_of(user: &User) -> i32 {
        user.id
    }
}

impl User {
    pub async fn by_permission(permission: Permission, connection: &mut PgConnection) -> Result<Vec<User>> {
        User::by_permissions(permission.bit(), connection).await
    }

    /// Gets all users that have the given permission bits all set
    pub async fn by_permissions(permissions: u16, connection: &mut PgConnection) -> Result<Vec<User>> {
        let mut stream = sqlx::query!(
            "SELECT member_id, name, permissions::integer, display_name, youtube_channel::text FROM members WHERE permissions & \
             CAST($1::INTEGER AS BIT(16)) = CAST($1::INTEGER AS BIT(16))",
            permissions as i32
        )
        .fetch(connection);

        let mut users = Vec::new();

        while let Some(row) = stream.next().await {
            let row = row?;

            users.push(User {
                id: row.member_id,
                name: row.name,
                permissions: row.permissions.unwrap() as u16,
                display_name: row.display_name,
                youtube_channel: row.youtube_channel,
            })
        }

        Ok(users)
    }
}
