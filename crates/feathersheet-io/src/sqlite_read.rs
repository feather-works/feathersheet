//! Leitor de banco SQLite (só leitura, sem biblioteca C), para o FeatherQuery (#70): lista as
//! tabelas e lê as linhas direto das páginas B-tree do arquivo (formato descrito em
//! <https://www.sqlite.org/fileformat2.html>).
//!
//! Fica de fora: views (pedem executar SQL), tabelas `WITHOUT ROWID` (são árvores de índice)
//! e o que ainda está só no `-wal` (o banco aberto por outro programa sem checkpoint).
// shortcut: lê o arquivo inteiro na memória; ler página a página do disco se aparecer banco de vários GB.
use feathersheet_core::celulas::values::Value;
use std::path::Path;

/// Uma tabela do banco: o nome, as colunas (na ordem do `CREATE TABLE`) e a página raiz.
#[derive(Debug, Clone, PartialEq)]
pub struct TabelaSqlite {
    pub nome: String,
    pub colunas: Vec<String>,
    raiz: u32,
    /// A coluna `INTEGER PRIMARY KEY`: o valor dela é o rowid (no registro vem nulo).
    coluna_rowid: Option<usize>,
}

pub struct BancoSqlite {
    bytes: Vec<u8>,
    pagina: usize,
    util: usize,
    utf16: Option<bool>,
}

fn u16_em(b: &[u8], i: usize) -> Option<usize> { Some(u16::from_be_bytes(b.get(i..i + 2)?.try_into().ok()?) as usize) }
fn u32_em(b: &[u8], i: usize) -> Option<u32> { Some(u32::from_be_bytes(b.get(i..i + 4)?.try_into().ok()?)) }

/// Inteiro de tamanho variável do SQLite: (valor, bytes lidos).
fn varint(b: &[u8], i: usize) -> Option<(u64, usize)> {
    let mut v: u64 = 0;
    for k in 0..9 {
        let byte = *b.get(i + k)?;
        if k == 8 { return Some(((v << 8) | u64::from(byte), 9)); }
        v = (v << 7) | u64::from(byte & 0x7f);
        if byte & 0x80 == 0 { return Some((v, k + 1)); }
    }
    None
}

fn erro() -> String { feathersheet_core::tr!("O arquivo SQLite está corrompido ou num formato que o FeatherSheet não lê.").into() }

impl BancoSqlite {
    pub fn abrir(caminho: &Path) -> Result<Self, String> {
        let bytes = std::fs::read(caminho).map_err(|e| feathersheet_core::trf!("Falha de leitura: {e}"))?;
        Self::de_bytes(bytes)
    }

    pub fn de_bytes(bytes: Vec<u8>) -> Result<Self, String> {
        if !bytes.starts_with(b"SQLite format 3\0") || bytes.len() < 100 {
            return Err(feathersheet_core::tr!("O arquivo não é um banco SQLite.").into());
        }
        let pagina = match u16_em(&bytes, 16).ok_or_else(erro)? { 1 => 65536, p => p };
        let util = pagina - usize::from(bytes[20]);
        let utf16 = match u32_em(&bytes, 56).ok_or_else(erro)? { 2 => Some(false), 3 => Some(true), _ => None };
        if !(512..=65536).contains(&pagina) || !pagina.is_power_of_two() || util < 480 { return Err(erro()); }
        Ok(Self { bytes, pagina, util, utf16 })
    }

    fn pagina(&self, n: u32) -> Option<&[u8]> {
        let ini = (n as usize).checked_sub(1)? * self.pagina;
        self.bytes.get(ini..ini + self.pagina)
    }

    /// O conteúdo de uma célula de folha de tabela, juntando as páginas de transbordo.
    fn carga(&self, pag: &[u8], i: usize) -> Option<(i64, Vec<u8>)> {
        let (total, n1) = varint(pag, i)?;
        let (rowid, n2) = varint(pag, i + n1)?;
        let total = usize::try_from(total).ok()?;
        let ini = i + n1 + n2;
        let x = self.util - 35;
        if total <= x { return Some((rowid as i64, pag.get(ini..ini + total)?.to_vec())); }
        let m = (self.util - 12) * 32 / 255 - 23;
        let k = m + (total - m) % (self.util - 4);
        let local = if k <= x { k } else { m };
        let mut dados = pag.get(ini..ini + local)?.to_vec();
        let mut prox = u32_em(pag, ini + local)?;
        let mut voltas = 0;
        while dados.len() < total && prox != 0 {
            voltas += 1;
            if voltas > self.bytes.len() / self.pagina { return None; } // laço no arquivo corrompido
            let p = self.pagina(prox)?;
            let falta = (total - dados.len()).min(self.util - 4);
            dados.extend_from_slice(p.get(4..4 + falta)?);
            prox = u32_em(p, 0)?;
        }
        (dados.len() == total).then_some((rowid as i64, dados))
    }

    /// Percorre as folhas da árvore da tabela, em ordem de rowid, até `parar` dizer chega.
    fn percorrer(&self, raiz: u32, parar: &mut dyn FnMut(i64, Vec<u8>) -> bool) -> Result<(), String> {
        let mut pilha = vec![raiz];
        let mut visitadas = 0usize;
        while let Some(n) = pilha.pop() {
            visitadas += 1;
            if visitadas > self.bytes.len() / self.pagina + 1 { return Err(erro()); }
            let pag = self.pagina(n).ok_or_else(erro)?;
            let h = if n == 1 { 100 } else { 0 };
            let tipo = *pag.get(h).ok_or_else(erro)?;
            let celulas = u16_em(pag, h + 3).ok_or_else(erro)?;
            match tipo {
                0x0d => for c in 0..celulas {
                    let p = u16_em(pag, h + 8 + 2 * c).ok_or_else(erro)?;
                    let (rowid, dados) = self.carga(pag, p).ok_or_else(erro)?;
                    if parar(rowid, dados) { return Ok(()); }
                },
                0x05 => {
                    // Pilha: o filho mais à direita sai por último.
                    pilha.push(u32_em(pag, h + 8).ok_or_else(erro)?);
                    for c in (0..celulas).rev() {
                        let p = u16_em(pag, h + 12 + 2 * c).ok_or_else(erro)?;
                        pilha.push(u32_em(pag, p).ok_or_else(erro)?);
                    }
                }
                _ => return Err(erro()),
            }
        }
        Ok(())
    }

    fn texto(&self, b: &[u8]) -> String {
        match self.utf16 {
            None => String::from_utf8_lossy(b).into_owned(),
            Some(be) => String::from_utf16_lossy(&b.chunks_exact(2)
                .map(|c| if be { u16::from_be_bytes([c[0], c[1]]) } else { u16::from_le_bytes([c[0], c[1]]) })
                .collect::<Vec<_>>()),
        }
    }

    /// Os valores de um registro, na ordem das colunas.
    fn registro(&self, dados: &[u8]) -> Option<Vec<Value>> {
        let (tam, n) = varint(dados, 0)?;
        let tam = usize::try_from(tam).ok()?;
        let (mut i, mut corpo) = (n, tam);
        let mut saida = Vec::new();
        while i < tam {
            let (tipo, n) = varint(dados, i)?;
            i += n;
            let inteiro = |len: usize, corpo: usize| -> Option<i64> {
                let b = dados.get(corpo..corpo + len)?;
                let mut v = if b[0] & 0x80 != 0 { -1i64 } else { 0 };
                for &x in b { v = (v << 8) | i64::from(x); }
                Some(v)
            };
            let (valor, len) = match tipo {
                0 => (Value::Empty, 0),
                1..=6 => {
                    let len = [0, 1, 2, 3, 4, 6, 8][tipo as usize];
                    let v = inteiro(len, corpo)?;
                    // Acima de 2^53 o número perde dígitos: vai como texto, inteiro.
                    (if v.unsigned_abs() > 1 << 53 { Value::Text(v.to_string()) } else { Value::Number(v as f64) }, len)
                }
                7 => (Value::Number(f64::from_bits(u64::from_be_bytes(dados.get(corpo..corpo + 8)?.try_into().ok()?))), 8),
                8 => (Value::Number(0.0), 0),
                9 => (Value::Number(1.0), 0),
                t if t >= 12 && t % 2 == 0 => {
                    let len = usize::try_from((t - 12) / 2).ok()?;
                    (Value::Text(feathersheet_core::trf!("[binário, {len} bytes]")), len)
                }
                t if t >= 13 => {
                    let len = usize::try_from((t - 13) / 2).ok()?;
                    (Value::Text(self.texto(dados.get(corpo..corpo + len)?)), len)
                }
                _ => return None,
            };
            corpo += len;
            saida.push(valor);
        }
        Some(saida)
    }

    /// As tabelas do banco (sem as internas `sqlite_*`, as views e as `WITHOUT ROWID`).
    pub fn tabelas(&self) -> Result<Vec<TabelaSqlite>, String> {
        let mut saida = Vec::new();
        let mut falha = None;
        self.percorrer(1, &mut |_, dados| {
            let Some(r) = self.registro(&dados) else { falha = Some(erro()); return true };
            let txt = |i: usize| match r.get(i) { Some(Value::Text(t)) => t.clone(), _ => String::new() };
            let raiz = match r.get(3) { Some(Value::Number(n)) => *n as u32, _ => 0 };
            let nome = txt(1);
            let sql = txt(4);
            if txt(0) == "table" && raiz > 0 && !nome.starts_with("sqlite_") && !sql.to_uppercase().contains("WITHOUT ROWID") {
                let (colunas, coluna_rowid) = colunas_do_create(&sql);
                saida.push(TabelaSqlite { nome, colunas, raiz, coluna_rowid });
            }
            false
        })?;
        falha.map_or(Ok(saida), Err)
    }

    /// As linhas da tabela (no máximo `limite`), com tantas colunas quanto o `CREATE TABLE`.
    pub fn linhas(&self, t: &TabelaSqlite, limite: Option<usize>) -> Result<Vec<Vec<Value>>, String> {
        let mut saida = Vec::new();
        let mut falha = None;
        self.percorrer(t.raiz, &mut |rowid, dados| {
            if limite.is_some_and(|l| saida.len() >= l) { return true; }
            let Some(mut linha) = self.registro(&dados) else { falha = Some(erro()); return true };
            // Coluna acrescentada com ALTER TABLE depois da linha gravada: vem vazia.
            linha.resize(t.colunas.len().max(1), Value::Empty);
            linha.truncate(t.colunas.len());
            if let Some(i) = t.coluna_rowid { if let Some(v) = linha.get_mut(i) { *v = Value::Number(rowid as f64); } }
            saida.push(linha);
            false
        })?;
        falha.map_or(Ok(saida), Err)
    }

    /// Quantas linhas a tabela tem (conta as células das folhas, sem decodificar).
    pub fn contar(&self, t: &TabelaSqlite) -> Result<usize, String> {
        let mut n = 0;
        self.percorrer(t.raiz, &mut |_, _| { n += 1; false })?;
        Ok(n)
    }
}

/// Nome sem as aspas do SQL: `"a b"`, `[a b]`, `` `a` ``, `'a'`.
fn sem_aspas(nome: &str) -> String {
    let n = nome.trim();
    let mut c = n.chars();
    match (c.next(), n.chars().last()) {
        (Some(a @ ('"' | '`' | '\'')), Some(b)) if a == b && n.len() >= 2 => n[1..n.len() - 1].replace(&format!("{a}{a}"), &a.to_string()),
        (Some('['), Some(']')) => n[1..n.len() - 1].to_owned(),
        _ => n.to_owned(),
    }
}

/// O primeiro nome da definição, que pode ter espaço dentro das aspas.
fn primeiro_nome(def: &str) -> (&str, &str) {
    let d = def.trim_start();
    let fim = match d.chars().next() {
        Some('"') => d[1..].find('"').map(|i| i + 2),
        Some('`') => d[1..].find('`').map(|i| i + 2),
        Some('[') => d.find(']').map(|i| i + 1),
        _ => d.find(char::is_whitespace),
    }.unwrap_or(d.len());
    (&d[..fim], &d[fim..])
}

/// As colunas do `CREATE TABLE` e qual delas é `INTEGER PRIMARY KEY` (o rowid).
fn colunas_do_create(sql: &str) -> (Vec<String>, Option<usize>) {
    let (Some(a), Some(b)) = (sql.find('('), sql.rfind(')')) else { return (Vec::new(), None) };
    let corpo = &sql[a + 1..b.max(a + 1)];
    let mut defs = Vec::new();
    let (mut nivel, mut ini, mut aspas) = (0i32, 0usize, None::<char>);
    for (i, c) in corpo.char_indices() {
        match (aspas, c) {
            (Some(q), _) if c == q || (q == '[' && c == ']') => aspas = None,
            (Some(_), _) => {}
            (None, '"' | '`' | '\'' | '[') => aspas = Some(c),
            (None, '(') => nivel += 1,
            (None, ')') => nivel -= 1,
            (None, ',') if nivel == 0 => { defs.push(&corpo[ini..i]); ini = i + 1; }
            _ => {}
        }
    }
    defs.push(&corpo[ini..]);
    let mut colunas = Vec::new();
    let mut rowid = None;
    for def in defs {
        let maiusc = def.trim().to_uppercase();
        if ["CONSTRAINT", "PRIMARY", "UNIQUE", "CHECK", "FOREIGN"].iter().any(|p| maiusc.starts_with(p)) || maiusc.is_empty() { continue; }
        let (nome, resto) = primeiro_nome(def);
        let resto = resto.trim().to_uppercase();
        if resto.starts_with("INTEGER") && resto.contains("PRIMARY KEY") && !resto.contains("DESC") {
            rowid = Some(colunas.len());
        }
        colunas.push(sem_aspas(nome));
    }
    (colunas, rowid)
}

#[cfg(test)]
mod testes {
    use super::*;
    const ARQUIVO: &[u8] = include_bytes!("../tests/arquivos/vendas.sqlite");

    #[test]
    fn lista_as_tabelas_e_le_linhas_com_rowid_transbordo_e_alter_table() {
        let b = BancoSqlite::de_bytes(ARQUIVO.to_vec()).unwrap();
        let t = b.tabelas().unwrap();
        let nomes: Vec<&str> = t.iter().map(|t| t.nome.as_str()).collect();
        assert_eq!(nomes, ["vendas", "Itens do pedido"], "sem índice nem view");
        assert_eq!(t[0].colunas, ["id", "produto", "valor", "qtd", "nota fiscal", "obs"]);
        assert_eq!(b.contar(&t[0]).unwrap(), 300);
        let l = b.linhas(&t[0], None).unwrap();
        assert_eq!(l.len(), 300);
        assert_eq!(l[0][0], Value::Number(1.0), "INTEGER PRIMARY KEY vem do rowid");
        assert_eq!(l[0][1], Value::Text("Produto 1".into()));
        assert_eq!(l[1][2], Value::Number(3.0));
        assert_eq!(l[1][5], Value::Text("á".repeat(10)));
        assert_eq!(l[0][4], Value::Text("[binário, 3 bytes]".into()));
        assert_eq!(l[299][0], Value::Number(300.0), "ordem do rowid");
        // A linha 150 tem 3000+ caracteres: passa por páginas de transbordo.
        let Value::Text(grande) = &l[149][5] else { panic!() };
        assert!(grande.len() == 3003 && grande.ends_with("fim"));
        assert_eq!(b.linhas(&t[0], Some(20)).unwrap().len(), 20);
        let i = b.linhas(&t[1], None).unwrap();
        assert_eq!(t[1].colunas, ["pedido", "quantidade", "grande", "nova"]);
        assert_eq!(i[0], [Value::Number(1.0), Value::Number(2.0), Value::Text("9007199254740993".into()), Value::Empty]);
        assert_eq!(i[1][1], Value::Number(-5.0));
        assert_eq!(i[2][1], Value::Number(0.25));
        assert_eq!(i[2][3], Value::Text("n".into()));
    }

    #[test]
    fn recusa_o_que_nao_e_sqlite_e_arquivo_cortado() {
        assert!(BancoSqlite::de_bytes(b"PK\x03\x04 nada".to_vec()).is_err());
        let cortado = BancoSqlite::de_bytes(ARQUIVO[..3000].to_vec()).unwrap();
        assert!(cortado.tabelas().is_err() || cortado.tabelas().unwrap().iter().any(|t| cortado.linhas(t, None).is_err()));
    }
}
