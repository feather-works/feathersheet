//! FeatherQuery ▸ Obter dados ▸ Banco de dados (#70): PostgreSQL, MySQL/MariaDB e SQL Server. A conexão é a mesma
//! do FeatherDB (`featherdb-core/src/postgres_banco.rs` e `mysql_banco.rs`): crates `postgres` (protocolo
//! simples, o tipo da coluna pelo `prepare`) e `mysql` (Rust puro), TLS com `rustls`.
//!
//! - As tabelas e views viram opções do Navegador (`importador::Item`); com SQL escrito, uma só.
//! - Só leitura: Postgres dentro de `BEGIN TRANSACTION READ ONLY` … `ROLLBACK`; MySQL com a sessão
//!   `READ ONLY` (lá o `CREATE`/`DROP` fecha a transação sozinho e escaparia dela). O SQL escrito é um
//!   comando só (`um_comando`): um `COMMIT; DROP …` sairia do modo leitura.
//! - SQL Server (crate `tiberius`, Rust puro): não tem sessão só de leitura; cada consulta roda com
//!   `IMPLICIT_TRANSACTIONS ON` e termina em `ROLLBACK` (lá o DDL também volta), e o SQL escrito não
//!   pode ter `COMMIT` nem `IMPLICIT_TRANSACTIONS` (o T-SQL junta comandos sem `;`). Para garantia de
//!   verdade, um usuário só de leitura no servidor.
//! - A origem guarda servidor, porta, banco, usuário e o item, nunca a senha. "Lembrar a senha"
//!   a põe no cofre do sistema (crate `keyring`), como o FeatherDB.
// shortcut: traz a tabela inteira (até LIMITE_LINHAS) e filtra no FeatherQuery; levar filtro, colunas e
// limite para o servidor (WHERE/SELECT/LIMIT) quando tabelas grandes pesarem.
use super::*;
use std::sync::Arc;

/// O teto de linhas do Excel: acima disso, pedir um SQL com filtro.
const LIMITE_LINHAS: usize = 1_048_576;
/// Tabelas no Navegador (cada uma faz uma consulta de prévia).
const MAX_TABELAS: usize = 200;
const SERVICO_COFRE: &str = "FeatherSheet";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(super) enum Tipo { #[default] Postgres, MySql, SqlServer }

impl Tipo {
    pub const TODOS: [Tipo; 3] = [Tipo::Postgres, Tipo::MySql, Tipo::SqlServer];
    fn esquema(self) -> &'static str { match self { Tipo::Postgres => "postgres", Tipo::MySql => "mysql", Tipo::SqlServer => "sqlserver" } }
    pub fn porta_padrao(self) -> &'static str { match self { Tipo::Postgres => "5432", Tipo::MySql => "3306", Tipo::SqlServer => "1433" } }
    pub fn usuario_padrao(self) -> &'static str { match self { Tipo::Postgres => "postgres", Tipo::MySql => "root", Tipo::SqlServer => "sa" } }
}

impl std::fmt::Display for Tipo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(match self { Tipo::Postgres => "PostgreSQL", Tipo::MySql => "MySQL / MariaDB", Tipo::SqlServer => "SQL Server" })
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Conexao {
    pub tipo: Tipo,
    pub servidor: String,
    pub porta: String,
    pub banco: String,
    pub usuario: String,
    /// `sslmode=require`: criptografa sem conferir o certificado, como o `require` do Postgres.
    pub ssl: bool,
}

impl Default for Conexao {
    fn default() -> Self {
        Conexao { tipo: Tipo::Postgres, servidor: "localhost".into(), porta: "5432".into(), banco: String::new(), usuario: "postgres".into(), ssl: false }
    }
}

impl Conexao {
    fn porta_num(&self) -> u16 { self.porta.trim().parse().unwrap_or_else(|_| self.tipo.porta_padrao().parse().unwrap_or(5432)) }

    /// A chave no cofre e o começo da origem: `postgres://usuario@servidor:porta/banco` (ou `mysql://`).
    fn endereco(&self) -> String {
        format!("{}://{}@{}:{}/{}", self.tipo.esquema(), self.usuario.trim(), self.servidor.trim(), self.porta_num(), self.banco.trim())
    }

    /// "Banco: postgres://u@h:5432/db" (e `?ssl`). O item do Navegador vem depois de " ▸ ".
    pub fn origem(&self) -> String {
        format!("Banco: {}{}", self.endereco(), if self.ssl { "?ssl" } else { "" })
    }

    /// A conexão e o item escolhido (tabela "esquema.nome" ou "SQL: …") de uma origem gravada.
    pub fn da_origem(origem: &str) -> Option<(Self, Option<String>)> {
        let (base, item) = match origem.split_once(" ▸ ") { Some((b, i)) => (b, Some(i.to_owned())), None => (origem, None) };
        let resto = base.strip_prefix("Banco: ")?;
        let (esquema, resto) = resto.split_once("://")?;
        let tipo = Tipo::TODOS.into_iter().find(|t| t.esquema() == esquema)?;
        let (resto, ssl) = match resto.strip_suffix("?ssl") { Some(r) => (r, true), None => (resto, false) };
        let (usuario, resto) = resto.split_once('@')?;
        let (hospedeiro, banco) = resto.split_once('/')?;
        let (servidor, porta) = hospedeiro.rsplit_once(':')?;
        Some((Conexao { tipo, servidor: servidor.into(), porta: porta.into(), banco: banco.into(), usuario: usuario.into(), ssl }, item))
    }

    fn conectar(&self, senha: &str) -> Result<Cliente, String> {
        if self.servidor.trim().is_empty() || self.usuario.trim().is_empty() {
            return Err(tr!("Preencha o servidor e o usuário.").into());
        }
        if self.tipo == Tipo::SqlServer {
            let mut cfg = tiberius::Config::new();
            cfg.host(self.servidor.trim());
            cfg.port(self.porta_num());
            cfg.database(if self.banco.trim().is_empty() { "master" } else { self.banco.trim() });
            cfg.authentication(tiberius::AuthMethod::sql_server(self.usuario.trim(), senha));
            // Como o `require` dos outros: criptografa sem conferir o certificado (o do SQL Server costuma ser
            // autoassinado). Sem SSL, só o login vai criptografado.
            cfg.encryption(if self.ssl { tiberius::EncryptionLevel::Required } else { tiberius::EncryptionLevel::Off });
            cfg.trust_cert();
            let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(|e| e.to_string())?;
            let cliente = rt.block_on(async {
                use tokio_util::compat::TokioAsyncWriteCompatExt;
                let tcp = tokio::time::timeout(std::time::Duration::from_secs(15), tokio::net::TcpStream::connect(cfg.get_addr())).await
                    .map_err(|_| tr!("O servidor não respondeu.").to_string())?.map_err(|e| e.to_string())?;
                let _ = tcp.set_nodelay(true);
                let mut c = tiberius::Client::connect(cfg, tcp.compat_write()).await.map_err(|e| e.to_string())?;
                c.simple_query("SET IMPLICIT_TRANSACTIONS ON").await.map_err(|e| e.to_string())?
                    .into_results().await.map_err(|e| e.to_string())?;
                Ok::<_, String>(c)
            })?;
            return Ok(Cliente::Ms(rt, cliente));
        }
        if self.tipo == Tipo::MySql {
            let opts = mysql::OptsBuilder::new()
                .ip_or_hostname(Some(self.servidor.trim().to_owned())).tcp_port(self.porta_num())
                .user(Some(self.usuario.trim().to_owned())).pass(Some(senha.to_owned()))
                .db_name((!self.banco.trim().is_empty()).then(|| self.banco.trim().to_owned()))
                .tcp_connect_timeout(Some(std::time::Duration::from_secs(15)))
                .ssl_opts(self.ssl.then(|| mysql::SslOpts::default().with_danger_accept_invalid_certs(true)));
            let mut c = mysql::Conn::new(opts).map_err(|e| e.to_string())?;
            use mysql::prelude::Queryable;
            c.query_drop("SET SESSION TRANSACTION READ ONLY").map_err(|e| e.to_string())?;
            return Ok(Cliente::My(c));
        }
        let mut cfg = postgres::Config::new();
        cfg.host(self.servidor.trim()).port(self.porta_num()).user(self.usuario.trim()).password(senha)
            .application_name("FeatherSheet").connect_timeout(std::time::Duration::from_secs(15));
        cfg.dbname(if self.banco.trim().is_empty() { "postgres" } else { self.banco.trim() });
        let cliente = if self.ssl {
            cfg.ssl_mode(postgres::config::SslMode::Require);
            cfg.connect(tokio_postgres_rustls::MakeRustlsConnect::new(tls_sem_conferir()?))
        } else {
            cfg.connect(postgres::NoTls)
        };
        cliente.map(Cliente::Pg).map_err(|e| erro(&e))
    }
}

enum Cliente {
    Pg(postgres::Client),
    My(mysql::Conn),
    /// O tiberius é assíncrono: a conexão leva o runtime dela.
    Ms(tokio::runtime::Runtime, tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>),
}

/// O SQL escrito pela pessoa: um comando só (um `;` no fim pode), fora de aspas e comentários.
/// No SQL Server também sem `COMMIT`/`IMPLICIT_TRANSACTIONS` (sairiam da transação desfeita no fim).
fn sql_da_pessoa(tipo: Tipo, sql: &str) -> Result<&str, String> {
    let sql = um_comando(sql)?;
    if tipo == Tipo::SqlServer {
        let mut fora = String::new();
        let mut aspas = None::<char>;
        for c in sql.chars() {
            match (aspas, c) {
                (Some(q), _) if c == q || (q == '[' && c == ']') => aspas = None,
                (Some(_), _) => {}
                (None, '\'' | '"' | '[') => aspas = Some(c),
                (None, c) => fora.push(c.to_ascii_uppercase()),
            }
        }
        let palavras: Vec<&str> = fora.split(|c: char| !c.is_alphanumeric() && c != '_').collect();
        if palavras.iter().any(|p| matches!(*p, "COMMIT" | "IMPLICIT_TRANSACTIONS")) {
            return Err(tr!("Sem COMMIT nem IMPLICIT_TRANSACTIONS: a consulta só lê.").into());
        }
    }
    Ok(sql)
}

fn um_comando(sql: &str) -> Result<&str, String> {
    let sql = sql.trim().trim_end_matches(';').trim_end();
    let (mut aspas, mut chars) = (None::<char>, sql.chars().peekable());
    while let Some(c) = chars.next() {
        match (aspas, c) {
            (Some(q), _) if c == q => aspas = None,
            (Some(_), _) => {}
            (None, '\'' | '"' | '`') => aspas = Some(c),
            (None, '-') if chars.peek() == Some(&'-') => aspas = Some('\n'),
            (None, ';') => return Err(tr!("Escreva um comando SQL só (sem ; no meio).").into()),
            _ => {}
        }
    }
    Ok(sql)
}

/// A mensagem do servidor (`ERROR: …`), sem o "db error:" do driver na frente.
fn erro(e: &postgres::Error) -> String {
    e.as_db_error().map_or_else(|| e.to_string(), |d| d.to_string())
}

/// TLS que criptografa e confere a assinatura do aperto de mão, sem conferir quem é o servidor
/// (o `sslmode=require`); igual ao `tls::SemConferir` do FeatherDB.
fn tls_sem_conferir() -> Result<rustls::ClientConfig, String> {
    use rustls::client::danger::{HandshakeSignatureValid, ServerCertVerified, ServerCertVerifier};
    use rustls::crypto::{verify_tls12_signature, verify_tls13_signature, CryptoProvider};
    use rustls::pki_types::{CertificateDer, ServerName, UnixTime};
    use rustls::{DigitallySignedStruct, SignatureScheme};
    #[derive(Debug)]
    struct SemConferir(Arc<CryptoProvider>);
    impl ServerCertVerifier for SemConferir {
        fn verify_server_cert(&self, _: &CertificateDer<'_>, _: &[CertificateDer<'_>], _: &ServerName<'_>, _: &[u8], _: UnixTime) -> Result<ServerCertVerified, rustls::Error> {
            Ok(ServerCertVerified::assertion())
        }
        fn verify_tls12_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls12_signature(m, c, d, &self.0.signature_verification_algorithms)
        }
        fn verify_tls13_signature(&self, m: &[u8], c: &CertificateDer<'_>, d: &DigitallySignedStruct) -> Result<HandshakeSignatureValid, rustls::Error> {
            verify_tls13_signature(m, c, d, &self.0.signature_verification_algorithms)
        }
        fn supported_verify_schemes(&self) -> Vec<SignatureScheme> { self.0.signature_verification_algorithms.supported_schemes() }
    }
    let provedor = Arc::new(rustls::crypto::ring::default_provider());
    Ok(rustls::ClientConfig::builder_with_provider(provedor.clone()).with_safe_default_protocol_versions().map_err(|e| e.to_string())?
        .dangerous().with_custom_certificate_verifier(Arc::new(SemConferir(provedor))).with_no_client_auth())
}

/// Texto do protocolo simples → valor da célula, pelo tipo da coluna (nomes do Postgres e do MySQL).
fn valor(texto: Option<&str>, tipo: &str) -> Value {
    let Some(t) = texto else { return Value::Empty };
    match tipo {
        "int2" | "int4" | "int8" | "oid" | "float4" | "float8" | "numeric"
        | "tiny" | "short" | "long" | "longlong" | "int24" | "year" | "float" | "double" | "decimal" | "newdecimal" => match t.parse::<f64>() {
            // Inteiro grande demais para o f64 não perder dígitos: fica texto.
            Ok(n) if n.is_finite() && (n.abs() < 9.007_199_254_740_992e15 || t.contains(['.', 'e', 'E'])) => Value::Number(n),
            _ => Value::Text(t.to_owned()),
        },
        "bool" => Value::Bool(t == "t"),
        _ => Value::Text(t.to_owned()),
    }
}

/// Roda o SQL só para ler e devolve a tabela.
fn consultar(cliente: &mut Cliente, sql: &str) -> Result<Tabela, String> {
    match cliente {
        Cliente::Pg(c) => consultar_pg(c, sql),
        Cliente::My(c) => consultar_my(c, sql),
        Cliente::Ms(rt, c) => rt.block_on(consultar_ms(c, sql)),
    }
}

/// Célula do SQL Server → valor (datas como texto, como no Postgres e no MySQL).
fn valor_ms(d: &tiberius::ColumnData<'static>) -> Value {
    use tiberius::{ColumnData as D, FromSql};
    use tiberius::time::chrono::{NaiveDate, NaiveDateTime, NaiveTime};
    let numero = |n: f64| Value::Number(n);
    let data = |t: Option<String>| t.map_or(Value::Empty, Value::Text);
    match d {
        D::U8(v) => v.map_or(Value::Empty, |n| numero(n.into())),
        D::I16(v) => v.map_or(Value::Empty, |n| numero(n.into())),
        D::I32(v) => v.map_or(Value::Empty, |n| numero(n.into())),
        D::I64(v) => v.map_or(Value::Empty, |n| if n.unsigned_abs() > 1 << 53 { Value::Text(n.to_string()) } else { numero(n as f64) }),
        D::F32(v) => v.map_or(Value::Empty, |n| numero(n.into())),
        D::F64(v) => v.map_or(Value::Empty, numero),
        D::Bit(v) => v.map_or(Value::Empty, Value::Bool),
        D::String(v) => v.as_ref().map_or(Value::Empty, |t| Value::Text(t.to_string())),
        D::Guid(v) => v.map_or(Value::Empty, |g| Value::Text(g.to_string())),
        D::Binary(v) => v.as_ref().map_or(Value::Empty, |b| { let len = b.len(); Value::Text(feathersheet_core::trf!("[binário, {len} bytes]")) }),
        D::Numeric(v) => v.map_or(Value::Empty, |n| valor(Some(&n.to_string()), "numeric")),
        D::Xml(v) => v.as_ref().map_or(Value::Empty, |x| Value::Text(x.to_string())),
        D::Date(_) => data(NaiveDate::from_sql(d).ok().flatten().map(|t| t.format("%Y-%m-%d").to_string())),
        D::Time(_) => data(NaiveTime::from_sql(d).ok().flatten().map(|t| t.format("%H:%M:%S").to_string())),
        D::DateTimeOffset(_) => data(tiberius::time::chrono::DateTime::<chrono::Utc>::from_sql(d).ok().flatten()
            .map(|t| t.format("%Y-%m-%d %H:%M:%S").to_string())),
        _ => data(NaiveDateTime::from_sql(d).ok().flatten().map(|t| {
            if t.time() == NaiveTime::MIN { t.format("%Y-%m-%d").to_string() } else { t.format("%Y-%m-%d %H:%M:%S").to_string() }
        })),
    }
}

async fn consultar_ms(c: &mut tiberius::Client<tokio_util::compat::Compat<tokio::net::TcpStream>>, sql: &str) -> Result<Tabela, String> {
    let lido = async {
        let mut fluxo = c.simple_query(sql).await?;
        let cabecalho: Vec<String> = fluxo.columns().await?.map(|cs| cs.iter().map(|c| c.name().to_owned()).collect()).unwrap_or_default();
        let linhas = fluxo.into_first_result().await?;
        Ok::<_, tiberius::error::Error>((cabecalho, linhas))
    }.await;
    // Tudo que a consulta tenha mexido volta (IMPLICIT_TRANSACTIONS ON abriu a transação).
    if let Ok(f) = c.simple_query("IF @@TRANCOUNT > 0 ROLLBACK").await { let _ = f.into_results().await; }
    let (cabecalho, linhas) = lido.map_err(|e| e.to_string())?;
    if cabecalho.is_empty() { return Err(tr!("O SQL não devolveu linhas (use um SELECT).").into()); }
    if linhas.len() > LIMITE_LINHAS {
        return Err(trf!("A consulta passa de {LIMITE_LINHAS} linhas: escreva um SQL com filtro."));
    }
    Ok(Tabela { cabecalho, linhas: linhas.into_iter().map(|r| r.into_iter().map(|d| valor_ms(&d)).collect()).collect() })
}

fn consultar_my(cliente: &mut mysql::Conn, sql: &str) -> Result<Tabela, String> {
    use mysql::prelude::Queryable;
    let mut lido = cliente.query_iter(sql).map_err(|e| e.to_string())?;
    let mut t = Tabela { cabecalho: Vec::new(), linhas: Vec::new() };
    if let Some(conjunto) = lido.iter() {
        let colunas = conjunto.columns();
        t.cabecalho = colunas.as_ref().iter().map(|c| c.name_str().into_owned()).collect();
        // MYSQL_TYPE_NEWDECIMAL → "newdecimal", o nome que `valor` conhece.
        let tipos: Vec<String> = colunas.as_ref().iter()
            .map(|c| format!("{:?}", c.column_type()).trim_start_matches("MYSQL_TYPE_").to_lowercase()).collect();
        for linha in conjunto {
            if t.linhas.len() >= LIMITE_LINHAS {
                return Err(trf!("A consulta passa de {LIMITE_LINHAS} linhas: escreva um SQL com filtro."));
            }
            let linha = linha.map_err(|e| e.to_string())?;
            t.linhas.push(linha.unwrap().into_iter().enumerate().map(|(i, v)| {
                let texto = match v {
                    mysql::Value::NULL => None,
                    mysql::Value::Bytes(b) => Some(String::from_utf8_lossy(&b).into_owned()),
                    outro => Some(outro.as_sql(true).trim_matches('\'').to_owned()),
                };
                valor(texto.as_deref(), tipos.get(i).map_or("", String::as_str))
            }).collect());
        }
    }
    if t.cabecalho.is_empty() { return Err(tr!("O SQL não devolveu linhas (use um SELECT).").into()); }
    Ok(t)
}

fn consultar_pg(cliente: &mut postgres::Client, sql: &str) -> Result<Tabela, String> {
    cliente.batch_execute("BEGIN TRANSACTION READ ONLY").map_err(|e| erro(&e))?;
    let tipos: Vec<String> = cliente.prepare(sql)
        .map(|st| st.columns().iter().map(|c| c.type_().name().to_owned()).collect()).unwrap_or_default();
    let lido = cliente.simple_query(sql);
    let _ = cliente.batch_execute("ROLLBACK");
    let mut t = Tabela { cabecalho: Vec::new(), linhas: Vec::new() };
    for m in lido.map_err(|e| erro(&e))? {
        match m {
            postgres::SimpleQueryMessage::RowDescription(cols) => {
                t = Tabela { cabecalho: cols.iter().map(|c| c.name().to_owned()).collect(), linhas: Vec::new() };
            }
            postgres::SimpleQueryMessage::Row(linha) => {
                if t.linhas.len() >= LIMITE_LINHAS {
                    return Err(trf!("A consulta passa de {LIMITE_LINHAS} linhas: escreva um SQL com filtro."));
                }
                t.linhas.push((0..linha.len()).map(|i| valor(linha.get(i), tipos.get(i).map_or("", String::as_str))).collect());
            }
            _ => {}
        }
    }
    if t.cabecalho.is_empty() { return Err(tr!("O SQL não devolveu linhas (use um SELECT).").into()); }
    Ok(t)
}

fn citar(tipo: Tipo, nome: &str) -> String {
    match tipo {
        Tipo::Postgres => format!("\"{}\"", nome.replace('"', "\"\"")),
        Tipo::MySql => format!("`{}`", nome.replace('`', "``")),
        Tipo::SqlServer => format!("[{}]", nome.replace(']', "]]")),
    }
}

/// As 8 primeiras linhas: `TOP` no SQL Server, `LIMIT` nos outros.
fn previa(tipo: Tipo, tabela: &str) -> String {
    match tipo {
        Tipo::SqlServer => format!("SELECT TOP 8 * FROM {tabela}"),
        _ => format!("SELECT * FROM {tabela} LIMIT 8"),
    }
}

/// Lê uma opção do Navegador (abre outra conexão: a da listagem já fechou).
pub(super) fn ler_item(c: &Conexao, senha: &str, sql: &str) -> Result<Tabela, String> {
    consultar(&mut c.conectar(senha)?, sql)
}

/// As opções do Navegador: as tabelas e views do banco, ou só o resultado do SQL escrito.
pub(super) fn listar(c: &Conexao, senha: &str, sql: &str) -> Result<(Vec<importador::Item>, String), String> {
    let mut cliente = c.conectar(senha)?;
    let origem = c.origem();
    if !sql.trim().is_empty() {
        let sql = sql_da_pessoa(c.tipo, sql)?;
        let t = consultar(&mut cliente, sql)?;
        return Ok((vec![importador::Item::pronto(format!("SQL: {sql}"), t, Formatos::new())], origem));
    }
    let catalogo = consultar(&mut cliente, &match c.tipo {
        Tipo::Postgres => format!(
            "SELECT n.nspname, c.relname, greatest(c.reltuples, 0)::bigint FROM pg_class c \
             JOIN pg_namespace n ON n.oid = c.relnamespace \
             WHERE c.relkind IN ('r','v','m','p','f') AND n.nspname NOT IN ('pg_catalog','information_schema') \
             AND n.nspname NOT LIKE 'pg_toast%' AND has_table_privilege(c.oid, 'SELECT') \
             ORDER BY 1, 2 LIMIT {MAX_TABELAS}"),
        // Sem banco escolhido, todos os que o usuário vê (menos os do sistema).
        Tipo::MySql => format!(
            "SELECT table_schema, table_name, COALESCE(table_rows, 0) FROM information_schema.tables \
             WHERE table_schema NOT IN ('mysql','information_schema','performance_schema','sys') \
             AND (DATABASE() IS NULL OR table_schema = DATABASE()) ORDER BY 1, 2 LIMIT {MAX_TABELAS}"),
        Tipo::SqlServer => format!(
            "SELECT TOP {MAX_TABELAS} s.name, o.name, COALESCE(SUM(p.rows), 0) FROM sys.objects o \
             JOIN sys.schemas s ON s.schema_id = o.schema_id \
             LEFT JOIN sys.partitions p ON p.object_id = o.object_id AND p.index_id IN (0, 1) \
             WHERE o.type IN ('U', 'V') AND o.is_ms_shipped = 0 GROUP BY s.name, o.name ORDER BY 1, 2"),
    })?;
    let mut itens = Vec::new();
    for linha in catalogo.linhas {
        let txt = |i: usize| linha.get(i).map(feathersheet_core::celulas::values::to_text).unwrap_or_default();
        let (esquema, nome) = (txt(0), txt(1));
        let linhas = match linha.get(2) { Some(Value::Number(n)) => *n as usize, _ => 0 };
        let tabela = format!("{}.{}", citar(c.tipo, &esquema), citar(c.tipo, &nome));
        let completo = format!("SELECT * FROM {tabela}");
        let previa = consultar(&mut cliente, &previa(c.tipo, &tabela))?;
        itens.push(importador::Item::banco(format!("{esquema}.{nome}"), previa, linhas, c.clone(), senha.to_owned(), completo));
    }
    if itens.is_empty() { return Err(tr!("O banco não tem tabelas que este usuário possa ler.").into()); }
    Ok((itens, origem))
}

/// O cofre roda numa thread própria: o `keyring` usa um runtime tokio só dele.
fn no_cofre<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> Option<T> {
    std::thread::spawn(f).join().ok()
}

pub(super) fn senha_guardada(c: &Conexao) -> Option<String> {
    let conta = c.endereco();
    no_cofre(move || keyring::Entry::new(SERVICO_COFRE, &conta).ok()?.get_password().ok()).flatten()
}

pub(super) fn guardar_senha(c: &Conexao, senha: &str) -> Result<(), String> {
    let (conta, senha) = (c.endereco(), senha.to_owned());
    no_cofre(move || keyring::Entry::new(SERVICO_COFRE, &conta).and_then(|e| e.set_password(&senha)).map_err(|e| e.to_string()))
        .unwrap_or_else(|| Err(tr!("O cofre de senhas do sistema não respondeu.").into()))
}

/// O formulário da conexão (Obter dados ▸ Banco de dados PostgreSQL).
#[derive(Debug, Clone, Default)]
pub(super) struct Formulario {
    pub conexao: Conexao,
    pub senha: String,
    pub sql: String,
    pub lembrar: bool,
    pub aviso: String,
}

#[derive(Debug, Clone)]
pub enum Campo { Servidor, Porta, Banco, Usuario, Senha, Sql }

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn origem_vai_e_volta_sem_a_senha() {
        let c = Conexao { tipo: Tipo::Postgres, servidor: "db.empresa".into(), porta: "6543".into(), banco: "vendas".into(), usuario: "ana".into(), ssl: true };
        let o = format!("{} ▸ public.pedidos", c.origem());
        assert_eq!(o, "Banco: postgres://ana@db.empresa:6543/vendas?ssl ▸ public.pedidos");
        assert_eq!(Conexao::da_origem(&o), Some((c, Some("public.pedidos".into()))));
        let (_, item) = Conexao::da_origem("Banco: postgres://a@h:5432/db ▸ SQL: select 1 ▸ x").unwrap();
        assert_eq!(item.as_deref(), Some("SQL: select 1 ▸ x"));
        assert!(Conexao::da_origem("Arquivo: x.db").is_none());
        let (m, _) = Conexao::da_origem("Banco: mysql://root@h:3306/loja").unwrap();
        assert_eq!((m.tipo, m.porta.as_str()), (Tipo::MySql, "3306"));
        assert_eq!(m.origem(), "Banco: mysql://root@h:3306/loja");
        assert!(Conexao::da_origem("Banco: oracle://a@h:1/x").is_none());
    }

    #[test]
    fn valor_pelo_tipo_da_coluna() {
        assert_eq!(valor(Some("42"), "int4"), Value::Number(42.0));
        assert_eq!(valor(Some("12.50"), "numeric"), Value::Number(12.5));
        assert_eq!(valor(Some("9007199254740993"), "int8"), Value::Text("9007199254740993".into()));
        assert_eq!(valor(Some("t"), "bool"), Value::Bool(true));
        assert_eq!(valor(Some("2026-11-13"), "date"), Value::Text("2026-11-13".into()));
        assert_eq!(valor(None, "text"), Value::Empty);
        assert_eq!(citar(Tipo::Postgres, "a\"b"), "\"a\"\"b\"");
        assert_eq!(citar(Tipo::MySql, "a`b"), "`a``b`");
        assert_eq!(valor(Some("12.50"), "newdecimal"), Value::Number(12.5));
    }

    #[test]
    fn sql_escrito_e_um_comando_so() {
        assert_eq!(um_comando(" select 1; ").unwrap(), "select 1");
        assert_eq!(um_comando("select ';' -- a;b\n").unwrap(), "select ';' -- a;b");
        assert!(um_comando("commit; drop table x").is_err());
        assert!(um_comando("select 1; select 2").is_err());
        assert!(sql_da_pessoa(Tipo::SqlServer, "select 1 commit drop table x").is_err());
        assert!(sql_da_pessoa(Tipo::SqlServer, "set implicit_transactions off drop table x").is_err());
        assert!(sql_da_pessoa(Tipo::SqlServer, "select [commit], 'commit' from t").is_ok());
        assert_eq!(citar(Tipo::SqlServer, "a]b"), "[a]]b]");
        assert_eq!(previa(Tipo::SqlServer, "[dbo].[t]"), "SELECT TOP 8 * FROM [dbo].[t]");
    }

    /// Usa o cofre de verdade da máquina: `cargo test -- --ignored cofre_ida_e_volta`.
    #[test]
    #[ignore]
    fn cofre_ida_e_volta() {
        let c = Conexao { servidor: "featherquery-teste.invalid".into(), usuario: "teste".into(), ..Conexao::default() };
        guardar_senha(&c, "s3nh@ção").unwrap();
        assert_eq!(senha_guardada(&c).as_deref(), Some("s3nh@ção"));
        let conta = c.endereco();
        no_cofre(move || keyring::Entry::new(SERVICO_COFRE, &conta).unwrap().delete_credential().unwrap());
        assert_eq!(senha_guardada(&c), None);
    }

    /// Contra um banco de verdade: `FEATHERSHEET_TESTE_PG=postgres://usuario:senha@host:5432/banco` (ou `mysql://`)
    /// cargo test -- --ignored postgres_de_verdade` (a senha só na variável, nunca em arquivo).
    #[test]
    #[ignore]
    fn postgres_de_verdade() {
        let url = std::env::var("FEATHERSHEET_TESTE_PG").expect("FEATHERSHEET_TESTE_PG");
        let (esquema, resto) = url.split_once("://").unwrap();
        let (cred, resto) = resto.split_once('@').unwrap();
        let (usuario, senha) = cred.split_once(':').unwrap();
        let (c, _) = Conexao::da_origem(&format!("Banco: {esquema}://{usuario}@{resto}")).unwrap();
        let (itens, _) = listar(&c, senha, "").unwrap();
        assert!(!itens.is_empty());
        let sql = if c.tipo == Tipo::SqlServer { "SELECT 1 AS um, 'x' AS dois, CAST(1 AS BIT) AS tres" } else { "SELECT 1 AS um, 'x' AS dois, true AS tres" };
        let (t, _) = listar(&c, senha, sql).unwrap();
        let (t, _) = t.into_iter().next().unwrap().ler().unwrap();
        // O MySQL não tem lógico: `true` vem 1.
        let verdade = if c.tipo == Tipo::MySql { Value::Number(1.0) } else { Value::Bool(true) };
        assert_eq!(t.linhas[0], [Value::Number(1.0), Value::Text("x".into()), verdade]);
        assert!(listar(&c, senha, "CREATE TABLE featherq_nao (a int)").is_err(), "só leitura");
        assert!(listar(&c, senha, "SELECT 1; CREATE TABLE featherq_nao (a int)").is_err(), "um comando só");
        for i in itens {
            let (nome, linhas) = (i.nome.clone(), i.linhas);
            let (t, _) = i.ler().unwrap();
            eprintln!("{nome}: {linhas} estimadas, {} lidas, {:?} | {:?}", t.linhas.len(), t.cabecalho, t.linhas.first());
        }
    }
}
