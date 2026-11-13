//! FeatherQuery ▸ Obter dados ▸ Banco de dados PostgreSQL (#70). A conexão é a mesma do FeatherDB
//! (`featherdb-core/src/postgres_banco.rs`): crate `postgres` pelo protocolo simples (todo valor vem
//! como texto; o tipo da coluna, pelo `prepare`, decide número, lógico ou texto) e TLS com `rustls`.
//!
//! - As tabelas e views viram opções do Navegador (`importador::Item`); com SQL escrito, uma só.
//! - Tudo roda dentro de `BEGIN TRANSACTION READ ONLY` … `ROLLBACK`: nada escreve no banco.
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
const PREFIXO: &str = "Banco: postgres://";
const SERVICO_COFRE: &str = "FeatherSheet";

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Conexao {
    pub servidor: String,
    pub porta: String,
    pub banco: String,
    pub usuario: String,
    /// `sslmode=require`: criptografa sem conferir o certificado, como o `require` do Postgres.
    pub ssl: bool,
}

impl Default for Conexao {
    fn default() -> Self {
        Conexao { servidor: "localhost".into(), porta: "5432".into(), banco: String::new(), usuario: "postgres".into(), ssl: false }
    }
}

impl Conexao {
    fn porta_num(&self) -> u16 { self.porta.trim().parse().unwrap_or(5432) }

    /// A chave no cofre e o começo da origem: `postgres://usuario@servidor:porta/banco`.
    fn endereco(&self) -> String {
        format!("postgres://{}@{}:{}/{}", self.usuario.trim(), self.servidor.trim(), self.porta_num(), self.banco.trim())
    }

    /// "Banco: postgres://u@h:5432/db" (e `?ssl`). O item do Navegador vem depois de " ▸ ".
    pub fn origem(&self) -> String {
        format!("Banco: {}{}", self.endereco(), if self.ssl { "?ssl" } else { "" })
    }

    /// A conexão e o item escolhido (tabela "esquema.nome" ou "SQL: …") de uma origem gravada.
    pub fn da_origem(origem: &str) -> Option<(Self, Option<String>)> {
        let (base, item) = match origem.split_once(" ▸ ") { Some((b, i)) => (b, Some(i.to_owned())), None => (origem, None) };
        let resto = base.strip_prefix(PREFIXO)?;
        let (resto, ssl) = match resto.strip_suffix("?ssl") { Some(r) => (r, true), None => (resto, false) };
        let (usuario, resto) = resto.split_once('@')?;
        let (hospedeiro, banco) = resto.split_once('/')?;
        let (servidor, porta) = hospedeiro.rsplit_once(':')?;
        Some((Conexao { servidor: servidor.into(), porta: porta.into(), banco: banco.into(), usuario: usuario.into(), ssl }, item))
    }

    fn conectar(&self, senha: &str) -> Result<postgres::Client, String> {
        if self.servidor.trim().is_empty() || self.usuario.trim().is_empty() {
            return Err(tr!("Preencha o servidor e o usuário.").into());
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
        cliente.map_err(|e| erro(&e))
    }
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

/// Texto do protocolo simples → valor da célula, pelo tipo da coluna.
fn valor(texto: Option<&str>, tipo: &str) -> Value {
    let Some(t) = texto else { return Value::Empty };
    match tipo {
        "int2" | "int4" | "int8" | "oid" | "float4" | "float8" | "numeric" => match t.parse::<f64>() {
            // Inteiro grande demais para o f64 não perder dígitos: fica texto.
            Ok(n) if n.is_finite() && (n.abs() < 9.007_199_254_740_992e15 || t.contains(['.', 'e', 'E'])) => Value::Number(n),
            _ => Value::Text(t.to_owned()),
        },
        "bool" => Value::Bool(t == "t"),
        _ => Value::Text(t.to_owned()),
    }
}

/// Roda o SQL só para ler e devolve a tabela.
fn consultar(cliente: &mut postgres::Client, sql: &str) -> Result<Tabela, String> {
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

fn citar(nome: &str) -> String { format!("\"{}\"", nome.replace('"', "\"\"")) }

/// Lê uma opção do Navegador (abre outra conexão: a da listagem já fechou).
pub(super) fn ler_item(c: &Conexao, senha: &str, sql: &str) -> Result<Tabela, String> {
    consultar(&mut c.conectar(senha)?, sql)
}

/// As opções do Navegador: as tabelas e views do banco, ou só o resultado do SQL escrito.
pub(super) fn listar(c: &Conexao, senha: &str, sql: &str) -> Result<(Vec<importador::Item>, String), String> {
    let mut cliente = c.conectar(senha)?;
    let origem = c.origem();
    if !sql.trim().is_empty() {
        let t = consultar(&mut cliente, sql.trim())?;
        return Ok((vec![importador::Item::pronto(format!("SQL: {}", sql.trim()), t, Formatos::new())], origem));
    }
    let catalogo = consultar(&mut cliente, &format!(
        "SELECT n.nspname, c.relname, greatest(c.reltuples, 0)::bigint FROM pg_class c \
         JOIN pg_namespace n ON n.oid = c.relnamespace \
         WHERE c.relkind IN ('r','v','m','p','f') AND n.nspname NOT IN ('pg_catalog','information_schema') \
         AND n.nspname NOT LIKE 'pg_toast%' AND has_table_privilege(c.oid, 'SELECT') \
         ORDER BY 1, 2 LIMIT {MAX_TABELAS}"))?;
    let mut itens = Vec::new();
    for linha in catalogo.linhas {
        let txt = |i: usize| linha.get(i).map(feathersheet_core::celulas::values::to_text).unwrap_or_default();
        let (esquema, nome) = (txt(0), txt(1));
        let linhas = match linha.get(2) { Some(Value::Number(n)) => *n as usize, _ => 0 };
        let completo = format!("SELECT * FROM {}.{}", citar(&esquema), citar(&nome));
        let previa = consultar(&mut cliente, &format!("{completo} LIMIT 8"))?;
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
        let c = Conexao { servidor: "db.empresa".into(), porta: "6543".into(), banco: "vendas".into(), usuario: "ana".into(), ssl: true };
        let o = format!("{} ▸ public.pedidos", c.origem());
        assert_eq!(o, "Banco: postgres://ana@db.empresa:6543/vendas?ssl ▸ public.pedidos");
        assert_eq!(Conexao::da_origem(&o), Some((c, Some("public.pedidos".into()))));
        let (_, item) = Conexao::da_origem("Banco: postgres://a@h:5432/db ▸ SQL: select 1 ▸ x").unwrap();
        assert_eq!(item.as_deref(), Some("SQL: select 1 ▸ x"));
        assert!(Conexao::da_origem("Arquivo: x.db").is_none());
    }

    #[test]
    fn valor_pelo_tipo_da_coluna() {
        assert_eq!(valor(Some("42"), "int4"), Value::Number(42.0));
        assert_eq!(valor(Some("12.50"), "numeric"), Value::Number(12.5));
        assert_eq!(valor(Some("9007199254740993"), "int8"), Value::Text("9007199254740993".into()));
        assert_eq!(valor(Some("t"), "bool"), Value::Bool(true));
        assert_eq!(valor(Some("2026-11-13"), "date"), Value::Text("2026-11-13".into()));
        assert_eq!(valor(None, "text"), Value::Empty);
        assert_eq!(citar("a\"b"), "\"a\"\"b\"");
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

    /// Contra um Postgres de verdade: `FEATHERSHEET_TESTE_PG=postgres://usuario:senha@host:5432/banco
    /// cargo test -- --ignored postgres_de_verdade` (a senha só na variável, nunca em arquivo).
    #[test]
    #[ignore]
    fn postgres_de_verdade() {
        let url = std::env::var("FEATHERSHEET_TESTE_PG").expect("FEATHERSHEET_TESTE_PG");
        let resto = url.strip_prefix("postgres://").unwrap();
        let (cred, resto) = resto.split_once('@').unwrap();
        let (usuario, senha) = cred.split_once(':').unwrap();
        let (c, _) = Conexao::da_origem(&format!("{PREFIXO}{usuario}@{resto}")).unwrap();
        let (itens, _) = listar(&c, senha, "").unwrap();
        assert!(!itens.is_empty());
        let (t, _) = listar(&c, senha, "SELECT 1 AS um, 'x' AS dois, true AS tres").unwrap();
        let (t, _) = t.into_iter().next().unwrap().ler().unwrap();
        assert_eq!(t.linhas[0], [Value::Number(1.0), Value::Text("x".into()), Value::Bool(true)]);
        assert!(listar(&c, senha, "CREATE TABLE featherq_nao (a int)").is_err(), "só leitura");
        for i in itens {
            let (nome, linhas) = (i.nome.clone(), i.linhas);
            let (t, _) = i.ler().unwrap();
            eprintln!("{nome}: {linhas} estimadas, {} lidas, {:?} | {:?}", t.linhas.len(), t.cabecalho, t.linhas.first());
        }
    }
}
