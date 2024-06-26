use crate::{
    nationality::{Continent, Nationality},
    player::{DatabasePlayer, Player, RankedPlayer},
};
use futures::StreamExt;
use pointercrate_core::{
    first_and_last,
    pagination::{PageContext, Pagination, PaginationParameters, __pagination_compat},
    util::{non_nullable, nullable},
};
use serde::{Deserialize, Serialize};
use sqlx::{postgres::PgConnection, Row};

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct PlayerPagination {
    #[serde(flatten)]
    pub params: PaginationParameters,

    #[serde(default, deserialize_with = "non_nullable")]
    name: Option<String>,

    #[serde(default, deserialize_with = "non_nullable")]
    name_contains: Option<String>,

    #[serde(default, deserialize_with = "non_nullable")]
    pub banned: Option<bool>,

    #[serde(default, deserialize_with = "nullable")]
    nation: Option<Option<String>>,
}

impl Pagination for PlayerPagination {
    type Item = Player;

    fn parameters(&self) -> PaginationParameters {
        self.params
    }

    fn with_parameters(&self, parameters: PaginationParameters) -> Self {
        Self {
            params: parameters,
            ..self.clone()
        }
    }

    first_and_last!("players");

    async fn page(&self, connection: &mut PgConnection) -> Result<(Vec<Player>, PageContext), sqlx::Error> {
        let order = self.params.order();

        let query = format!(include_str!("../../sql/paginate_players_by_id.sql"), order);

        // FIXME(sqlx) once CITEXT is supported
        let mut stream = sqlx::query(&query)
            .bind(self.params.before)
            .bind(self.params.after)
            .bind(self.name.as_deref())
            .bind(self.name_contains.as_deref())
            .bind(self.banned)
            .bind(&self.nation)
            .bind(self.nation == Some(None))
            .bind(self.params.limit + 1)
            .fetch(connection);

        let mut players = Vec::new();

        while let Some(row) = stream.next().await {
            let row = row?;

            let nationality = match (row.get("nation"), row.get("iso_country_code")) {
                (Some(nation), Some(country_code)) => Some(Nationality {
                    iso_country_code: country_code,
                    nation,
                    subdivision: None, // dont include subdivision in pagination data
                }),
                _ => None,
            };

            players.push(Player {
                base: DatabasePlayer {
                    id: row.get("id"),
                    name: row.get("name"),
                    banned: row.get("banned"),
                },
                nationality,
            })
        }

        Ok(__pagination_compat(&self.params, players))
    }

    fn id_of(item: &Self::Item) -> i32 {
        item.base.id
    }
}

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct RankingPagination {
    #[serde(flatten)]
    pub params: PaginationParameters,

    #[serde(default, deserialize_with = "nullable")]
    nation: Option<Option<String>>,

    #[serde(default, deserialize_with = "non_nullable")]
    continent: Option<Continent>,

    #[serde(default, deserialize_with = "non_nullable")]
    subdivision: Option<String>,

    #[serde(default, deserialize_with = "non_nullable")]
    name_contains: Option<String>,
}

impl Pagination for RankingPagination {
    type Item = RankedPlayer;

    fn parameters(&self) -> PaginationParameters {
        self.params
    }

    fn with_parameters(&self, parameters: PaginationParameters) -> Self {
        Self {
            params: parameters,
            ..self.clone()
        }
    }

    async fn first_and_last(connection: &mut PgConnection) -> Result<Option<(i32, i32)>, sqlx::Error> {
        Ok(sqlx::query!("SELECT MAX(index) FROM players_with_score")
            .fetch_one(connection)
            .await?
            .max
            .map(|max| (1, max as i32)))
    }

    async fn page(&self, connection: &mut PgConnection) -> Result<(Vec<RankedPlayer>, PageContext), sqlx::Error> {
        let order = self.params.order();

        let query = format!(include_str!("../../sql/paginate_player_ranking.sql"), order);

        let mut stream = sqlx::query(&query)
            .bind(self.params.before)
            .bind(self.params.after)
            .bind(self.name_contains.as_deref())
            .bind(&self.nation)
            .bind(self.nation == Some(None))
            .bind(self.continent.as_ref().map(|c| c.to_sql()))
            .bind(&self.subdivision)
            .bind(self.params.limit + 1)
            .fetch(connection);

        let mut players = Vec::new();

        while let Some(row) = stream.next().await {
            let row = row?;

            let nationality = match (row.get("nation"), row.get("iso_country_code")) {
                (Some(nation), Some(country_code)) => Some(Nationality {
                    iso_country_code: country_code,
                    nation,
                    subdivision: None, // dont include subdivision in pagination data
                }),
                _ => None,
            };

            players.push(RankedPlayer {
                id: row.get("id"),
                name: row.get("name"),
                rank: row.get("rank"),
                nationality,
                score: row.get("score"),
                index: row.get("index"),
            })
        }

        Ok(__pagination_compat(&self.params, players))
    }

    fn id_of(item: &Self::Item) -> i32 {
        item.index as i32
    }
}
