use darling::{FromDeriveInput, FromField};
use itertools::Itertools;
use proc_macro2::Span;
use quote::{quote, format_ident};
use syn::spanned::Spanned;
use syn::{parse2, DeriveInput};

#[derive(Debug, FromDeriveInput)]
#[darling(attributes(domain))]
struct DomainOpts {
    ident: syn::Ident,
    table: String,
    data: darling::ast::Data<darling::util::Ignored, DomainFieldOpt>,
}

#[derive(Debug, FromField)]
#[darling(attributes(domain))]
struct DomainFieldOpt {
    ident: Option<syn::Ident>,
    ty: syn::Type,
    #[darling(default)]
    primary_key: Option<bool>,
}

fn find_by_id(table_name: &str, primary_field_name: &str) -> String {
    format!("select * from {} where {} = $1", table_name, primary_field_name)
}
fn fetch_all(table_name: &str) -> String {
    format!("select * from {}", table_name)
}

fn delete_by_pk(table_name: &str, primary_field_name: &str) -> String {
    format!("delete from {} where {} = $1", table_name, primary_field_name)
}

pub(crate) fn handler(input: proc_macro2::TokenStream) -> Result<proc_macro2::TokenStream, (Span, &'static str)> {
    let x1 = parse2::<DeriveInput>(input).unwrap();
    let crud_opts: DomainOpts = DomainOpts::from_derive_input(&x1).unwrap();

    let fields = crud_opts.data.take_struct().unwrap();
    let non_pk_field_names = fields.fields.iter().filter(|field| field.primary_key.is_none()).filter_map(|field|field.ident.as_ref().map(|it|it.to_string())).map(|it| quote!{#it}).collect_vec();
    let non_pk_fields_count = non_pk_field_names.len();
    let mut pk_count = fields.fields.into_iter().filter(|field| field.primary_key == Some(true)).collect_vec();

    let pk_field = match pk_count.len() {
        0 => {
            return Err((x1.span(), "missing primary key, using #[domain(primary_key)] to identify"));
        }
        1 => pk_count.pop().unwrap(),
        _ => {
            return Err((x1.span(), "mutliple primary key detect"));
        }
    };
    let pk_field_name = pk_field.ident.unwrap().to_string();
    let pk_field_type = pk_field.ty;

    let table_name = &crud_opts.table;
    let ident = crud_opts.ident;
    let field_ident = format_ident!("__CONSERVATOR_{}_DOMAIN_FIELDS", ident.clone().to_string().to_uppercase());

    let find_by_id_sql = find_by_id(&crud_opts.table, &pk_field_name);
    let fetch_all_sql = fetch_all(&crud_opts.table);
    let delete_by_pk = delete_by_pk(&crud_opts.table, &pk_field_name);

    Ok(quote! {


        static #field_ident : [&'static; #non_pk_fields_count] = [#(#non_pk_field_names ,)*];
        #[async_trait::async_trait]
        impl ::conservator::Domain for #ident {
            const PK_FIELD_NAME: &'static str = #pk_field_name;
            const TABLE_NAME: &'static str = #table_name;

            type PrimaryKey = #pk_field_type;

            async fn find_by_pk<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database=::sqlx::Postgres>>(pk: &Uuid, executor: E) -> Result<Option<Self>, ::sqlx::Error> {
                sqlx::query_as(#find_by_id_sql)
                .bind(pk)
                .fetch_optional(executor)
                .await
            }

            async fn fetch_one_by_pk<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database=::sqlx::Postgres>>(pk: &Uuid, executor: E) -> Result<Self, ::sqlx::Error> {
                sqlx::query_as(#find_by_id_sql)
                .bind(pk)
                .fetch_one(executor)
                .await
            }

            async fn fetch_all<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database=::sqlx::Postgres>>(executor: E) -> Result<Vec<Self>, ::sqlx::Error> {
                sqlx::query_as(#fetch_all_sql)
                .fetch_all(executor)
                .await
            }
            async fn create<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>, C: ::conservator::Creatable>(
                data: C, executor: E
            ) -> Result<Self, ::sqlx::Error> {
                let sql = format!("INSERT INTO {} {} returning *", #table_name, data.get_insert_sql());
                let mut ex = sqlx::query_as(&sql);
                data.build(ex)
                    .fetch_one(executor)
                    .await
            }
            async fn delete_by_pk<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>>(pk: &Self::PrimaryKey, executor: E,) ->Result<(), ::sqlx::Error> {
                sqlx::query(#delete_by_pk)
                .bind(pk)
                .execute(executor)
                .await?;
                Ok(())
            }
        }

    })
}

#[cfg(test)]
mod test {
    use quote::quote;

    use crate::domain::handler;

    #[test]
    fn should_render() {
        let input = quote! {
            #[derive(Debug, Deserialize, Serialize, Domain, FromRow)]
            #[domain(table = "users")]
            pub struct UserEntity {
                #[domain(primary_key)]
                pub id: Uuid,
                pub username: String,
                pub email: String,
                pub password: String,
                pub role: UserRole,
                pub create_at: DateTime<Utc>,
                pub last_login_at: DateTime<Utc>,
            }
        };
        let expected_output = quote! {

            static __CONSERVATOR_USERENTITY_DOMAIN_FIELDS :[&'static; 6usize] = ["username" , "email" , "password" , "role" , "create_at" , "last_login_at" ,];
            #[async_trait::async_trait]
            impl ::conservator::Domain for UserEntity {
                const PK_FIELD_NAME: &'static str = "id";
                const TABLE_NAME: &'static str = "users";
                type PrimaryKey = Uuid;
                async fn find_by_pk<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>>(
                    pk: &Uuid,
                    executor: E
                ) -> Result<Option<Self>, ::sqlx::Error> {
                    sqlx::query_as("select * from users where id = $1")
                        .bind(pk)
                        .fetch_optional(executor)
                        .await
                }
                async fn fetch_one_by_pk<
                    'e,
                    'c: 'e,
                    E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>>(
                    pk: &Uuid,
                    executor: E
                ) -> Result<Self, ::sqlx::Error> {
                    sqlx::query_as("select * from users where id = $1")
                        .bind(pk)
                        .fetch_one(executor)
                        .await
                }
                async fn fetch_all<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>>(
                    executor: E
                ) -> Result<Vec<Self>, ::sqlx::Error> {
                    sqlx::query_as("select * from users")
                        .fetch_all(executor)
                        .await
                }
                async fn create<
                    'e,
                    'c: 'e,
                    E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>,
                    C: ::conservator::Creatable
                >(
                    data: C,
                    executor: E
                ) -> Result<Self, ::sqlx::Error> {
                    let sql = format!(
                        "INSERT INTO {} {} returning *",
                        "users",
                        data.get_insert_sql()
                    );
                    let mut ex = sqlx::query_as(&sql);
                    data.build(ex).fetch_one(executor).await
                }

            async fn delete_by_pk<'e, 'c: 'e, E: 'e + ::sqlx::Executor<'c, Database = ::sqlx::Postgres>>(pk: &Self::PrimaryKey, executor: E,) ->Result<(), ::sqlx::Error> {
                    sqlx::query("delete from users where id = $1")
                    .bind(pk)
                .execute(executor)
                .await?;
                    Ok(())
                }
            }
        };

        let stream = handler(input).unwrap();
        assert_eq!(expected_output.to_string(), stream.to_string());
    }
}
