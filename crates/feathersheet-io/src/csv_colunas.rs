//! Importação de CSV/TSV por colunas: o texto é dividido sem um `String` por
//! campo, cada coluna de um bloco de [`CHUNK_SIZE`] linhas é classificada
//! (número, texto, fórmula) e entregue pronta à aba, de uma vez, como o
//! `.fshz` faz com as colunas dele.
//!
//! Era célula por célula: ~300 ns de trabalho por célula, 21 s de CPU para
//! os 70 milhões de células do `spotify_train.csv` (653 MB, 16/10/2026).
//!
//! **Mesmas regras de antes.** A divisão segue as do
//! `feather_io::csv_lotes` (aspas, `""`, quebra de linha dentro de aspas,
//! CR, LF e CRLF, BOM), e cada campo vira o que o `set_cell_raw` faria com
//! ele: os dois atalhos de [`classificar`] só valem onde o resultado é o
//! mesmo do `parse_literal`; o resto passa por ele. Fórmulas (e o que não
//! for número, texto ou booleano) seguem pela carga célula a célula.

use std::collections::HashMap;
use std::hash::{BuildHasherDefault, Hasher};
use std::io::{BufRead, Read};

use feathersheet_core::celulas::input::parse_literal;
use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::planilha::column_store::{CHUNK_SHIFT, CHUNK_SIZE};
use feathersheet_core::planilha::sheet::carga::{CelulaPreparada, Padroes};
use feathersheet_core::planilha::sheet::Sheet;
use feathersheet_core::planilha::sheet::edicao::formato_do_literal;
use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};

/// As linhas de um bloco: o conteúdo dos campos um atrás do outro, onde
/// cada campo termina e onde cada linha começa (em campos).
struct BlocoLinhas {
    indice: usize,
    texto: Vec<u8>,
    fins: Vec<u32>,
    /// Primeiro campo de cada linha; tem uma entrada a mais que as linhas.
    linhas: Vec<u32>,
}

impl BlocoLinhas {
    fn novo(indice: usize) -> Self {
        BlocoLinhas { indice, texto: Vec::new(), fins: Vec::new(), linhas: vec![0] }
    }

    fn quantas_linhas(&self) -> usize {
        self.linhas.len() - 1
    }

    fn campo(&self, i: usize) -> (usize, usize) {
        (if i == 0 { 0 } else { self.fins[i - 1] as usize }, self.fins[i] as usize)
    }
}

/// O divisor como máquina de estados, alimentado aos pedaços: o estado
/// das aspas atravessa os pedaços e as linhas físicas.
struct Divisor {
    delim: u8,
    entre_aspas: bool,
    /// Aspa vista dentro de aspas: a próxima decide se é `""` ou o fim.
    aspa_pendente: bool,
    ignorar_lf: bool,
    inicio_do_arquivo: bool,
    inicio_campo: usize,
    bloco: BlocoLinhas,
    linhas_lidas: u64,
    excedeu: bool,
}

impl Divisor {
    fn novo(delim: u8) -> Self {
        Divisor {
            delim, entre_aspas: false, aspa_pendente: false, ignorar_lf: false,
            inicio_do_arquivo: true, inicio_campo: 0, bloco: BlocoLinhas::novo(0),
            linhas_lidas: 0, excedeu: false,
        }
    }

    fn campo_vazio(&self) -> bool {
        self.bloco.texto.len() == self.inicio_campo
    }

    fn fim_campo(&mut self) {
        self.bloco.fins.push(self.bloco.texto.len() as u32);
        self.inicio_campo = self.bloco.texto.len();
    }

    fn fim_linha(&mut self, entregar: &mut impl FnMut(BlocoLinhas)) {
        let inicio = *self.bloco.linhas.last().unwrap() as usize;
        if self.bloco.fins.len() - inicio > MAX_COLS as usize || self.linhas_lidas >= MAX_ROWS as u64 {
            self.excedeu = true;
        }
        self.bloco.linhas.push(self.bloco.fins.len() as u32);
        self.linhas_lidas += 1;
        if self.bloco.quantas_linhas() == CHUNK_SIZE {
            let proximo = BlocoLinhas::novo(self.bloco.indice + 1);
            entregar(std::mem::replace(&mut self.bloco, proximo));
            self.inicio_campo = 0;
        }
    }

    fn alimentar(&mut self, mut buf: &[u8], entregar: &mut impl FnMut(BlocoLinhas)) {
        if self.inicio_do_arquivo && !buf.is_empty() {
            // O BOM só conta no começo do arquivo; pode chegar aos pedaços,
            // mas o primeiro pedaço tem sempre mais de 3 bytes (64 KiB).
            buf = buf.strip_prefix(b"\xEF\xBB\xBF".as_slice()).unwrap_or(buf);
            self.inicio_do_arquivo = false;
        }
        let d = self.delim;
        let mut i = 0;
        while i < buf.len() {
            let b = buf[i];
            if std::mem::take(&mut self.ignorar_lf) && b == b'\n' {
                i += 1;
                continue;
            }
            if std::mem::take(&mut self.aspa_pendente) {
                if b == b'"' {
                    self.bloco.texto.push(b'"');
                    i += 1;
                    continue;
                }
                self.entre_aspas = false;
            }
            if self.entre_aspas {
                match buf[i..].iter().position(|&x| x == b'"') {
                    Some(p) => {
                        self.bloco.texto.extend_from_slice(&buf[i..i + p]);
                        self.aspa_pendente = true;
                        i += p + 1;
                    }
                    None => {
                        self.bloco.texto.extend_from_slice(&buf[i..]);
                        i = buf.len();
                    }
                }
                continue;
            }
            if b == b'"' && self.campo_vazio() {
                self.entre_aspas = true;
                i += 1;
            } else if b == d {
                self.fim_campo();
                i += 1;
            } else if b == b'\n' || b == b'\r' {
                self.ignorar_lf = b == b'\r';
                self.fim_campo();
                self.fim_linha(entregar);
                i += 1;
            } else {
                // Um trecho comum de uma vez, até o próximo separador ou fim
                // de linha (aspa no meio do campo é conteúdo).
                let p = buf[i..].iter().position(|&x| x == d || x == b'\n' || x == b'\r')
                    .unwrap_or(buf.len() - i);
                self.bloco.texto.extend_from_slice(&buf[i..i + p]);
                i += p;
            }
        }
    }

    /// Fim do arquivo: a última linha sem quebra no fim também conta.
    fn terminar(mut self, entregar: &mut impl FnMut(BlocoLinhas)) -> bool {
        self.aspa_pendente = false;
        let inicio = *self.bloco.linhas.last().unwrap() as usize;
        if !self.campo_vazio() || self.bloco.fins.len() > inicio {
            self.fim_campo();
            self.fim_linha(entregar);
        }
        if self.bloco.quantas_linhas() > 0 {
            entregar(self.bloco);
        }
        self.excedeu
    }
}

/// O que um campo é, sem montar texto novo.
enum Classe {
    Numero(f64),
    Texto,
    Bool(bool),
    /// Fórmula ou outro valor: vai pela carga célula a célula.
    Outra,
}

/// Mesmo resultado do `parse_literal` (já aparado), com atalhos exatos:
/// - `-123.45` é o número que o `f64` lê (é o primeiro caminho que o
///   `parse_number_str` toma para ele, e moeda exige `$`);
/// - sem nenhum dígito, só pode ser texto ou booleano: número, moeda,
///   percentual, data e hora exigem dígito;
/// - com uma letra ASCII fora de `e`/`E` (de `1e5`) e `r`/`R` (de `R$`), é
///   texto: nenhum daqueles formatos tem outra letra.
fn classificar(t: &str) -> Classe {
    if let Some(n) = numero_simples(t.as_bytes()) {
        return Classe::Numero(n);
    }
    let (mut digito, mut letra) = (false, false);
    for &c in t.as_bytes() {
        digito |= c.is_ascii_digit();
        letra |= c.is_ascii_alphabetic() && !matches!(c, b'e' | b'E' | b'r' | b'R');
    }
    if !digito || letra {
        if t.eq_ignore_ascii_case("VERDADEIRO") || t.eq_ignore_ascii_case("TRUE") {
            return Classe::Bool(true);
        }
        if t.eq_ignore_ascii_case("FALSO") || t.eq_ignore_ascii_case("FALSE") {
            return Classe::Bool(false);
        }
        return Classe::Texto;
    }
    let lido = parse_literal(t);
    if formato_do_literal(lido.format).is_some() {
        // Data, hora, %, moeda: o formato vai junto, pelo caminho célula a célula.
        return Classe::Outra;
    }
    match lido.value {
        Value::Number(n) => Classe::Numero(n),
        Value::Bool(v) => Classe::Bool(v),
        Value::Text(_) => Classe::Texto,
        _ => Classe::Outra,
    }
}

/// `-?[0-9]+(\.[0-9]+)?` lido como o `f64` lê. Até 15 dígitos e 22 casas,
/// dígitos ÷ 10^casas já é o valor certo (os dois são exatos no `f64` e a
/// divisão arredonda uma vez só); fora disso, o `parse` da biblioteca.
/// `None`: não tem esse formato (ou estoura o `f64`) — quem chama segue.
fn numero_simples(b: &[u8]) -> Option<f64> {
    const POTENCIAS: [f64; 23] = [1e0, 1e1, 1e2, 1e3, 1e4, 1e5, 1e6, 1e7, 1e8, 1e9, 1e10, 1e11,
        1e12, 1e13, 1e14, 1e15, 1e16, 1e17, 1e18, 1e19, 1e20, 1e21, 1e22];
    let (negativo, d) = match b.strip_prefix(b"-") { Some(r) => (true, r), None => (false, b) };
    let (mut mantissa, mut digitos, mut casas, mut i) = (0u64, 0usize, 0usize, 0usize);
    while i < d.len() && d[i].is_ascii_digit() {
        mantissa = mantissa.wrapping_mul(10).wrapping_add(u64::from(d[i] - b'0'));
        digitos += 1;
        i += 1;
    }
    if i == 0 {
        return None;
    }
    if i < d.len() {
        if d[i] != b'.' {
            return None;
        }
        i += 1;
        let inicio = i;
        while i < d.len() && d[i].is_ascii_digit() {
            mantissa = mantissa.wrapping_mul(10).wrapping_add(u64::from(d[i] - b'0'));
            digitos += 1;
            i += 1;
        }
        casas = i - inicio;
        if casas == 0 || i != d.len() {
            return None;
        }
    }
    if digitos <= 15 && casas <= 22 {
        let v = mantissa as f64 / POTENCIAS[casas];
        return Some(if negativo { -v } else { v });
    }
    std::str::from_utf8(b).ok()?.parse::<f64>().ok().filter(|n| n.is_finite())
}

/// Hash de multiplicar e girar, para o dicionário de textos de cada coluna
/// (o padrão da biblioteca é feito para resistir a ataque, não para isto).
#[derive(Default)]
struct HashRapido(u64);

impl Hasher for HashRapido {
    fn write(&mut self, bytes: &[u8]) {
        const K: u64 = 0x51_7c_c1_b7_27_22_0a_95;
        let mut pedacos = bytes.chunks_exact(8);
        for p in &mut pedacos {
            self.0 = (self.0.rotate_left(5) ^ u64::from_le_bytes(p.try_into().unwrap())).wrapping_mul(K);
        }
        for &b in pedacos.remainder() {
            self.0 = (self.0.rotate_left(5) ^ b as u64).wrapping_mul(K);
        }
    }

    fn write_u8(&mut self, b: u8) {
        self.write(&[b]);
    }

    fn finish(&self) -> u64 {
        self.0
    }
}

/// Entrada do dicionário: um trecho do texto do bloco, ou um booleano (que
/// a aba guarda como "VERDADEIRO"/"FALSO", igual ao `set_cell_value`).
#[derive(Clone, Copy)]
enum Verbete {
    Trecho(u32, u32),
    Bool(bool),
}

/// Uma coluna de um bloco, pronta para a aba.
struct BlocoColuna {
    col: u32,
    numeros: Option<(Box<[f64]>, Box<[u8]>)>,
    textos: Option<(Box<[i32]>, Box<[u8]>, Vec<Verbete>)>,
    /// Linha no bloco e trecho do texto: fórmulas e o resto.
    avulsas: Vec<(u32, u32, u32)>,
}

/// Monta uma coluna de um bloco, campo a campo (na ordem das linhas).
struct Construtor<'a> {
    numeros: Option<(Box<[f64]>, Box<[u8]>)>,
    textos: Option<(Box<[i32]>, Box<[u8]>)>,
    verbetes: Vec<Verbete>,
    dict: HashMap<&'a str, i32, BuildHasherDefault<HashRapido>>,
    bools: [i32; 2],
    avulsas: Vec<(u32, u32, u32)>,
}

impl<'a> Construtor<'a> {
    fn novo() -> Self {
        Construtor { numeros: None, textos: None, verbetes: Vec::new(), dict: HashMap::default(),
            bools: [-1; 2], avulsas: Vec::new() }
    }

    /// O campo `ini..fim` de `texto` na linha `r` do bloco.
    fn campo(&mut self, r: usize, texto: &'a str, ini: usize, fim: usize) {
        let t = texto[ini..fim].trim();
        if t.is_empty() {
            return;
        }
        let bit = 1u8 << (r & 7);
        let classe = if t.starts_with('=') { Classe::Outra } else { classificar(t) };
        let id = match classe {
            Classe::Numero(n) => {
                let (valores, mascara) = self.numeros.get_or_insert_with(|| (
                    vec![0.0; CHUNK_SIZE].into_boxed_slice(), vec![0u8; CHUNK_SIZE >> 3].into_boxed_slice()));
                valores[r] = n;
                mascara[r >> 3] |= bit;
                return;
            }
            Classe::Outra => {
                self.avulsas.push((r as u32, ini as u32, fim as u32));
                return;
            }
            Classe::Bool(v) => {
                let k = usize::from(!v);
                if self.bools[k] < 0 {
                    self.verbetes.push(Verbete::Bool(v));
                    self.bools[k] = self.verbetes.len() as i32 - 1;
                }
                self.bools[k]
            }
            Classe::Texto => {
                let verbetes = &mut self.verbetes;
                *self.dict.entry(t).or_insert_with(|| {
                    let inicio = (t.as_ptr() as usize - texto.as_ptr() as usize) as u32;
                    verbetes.push(Verbete::Trecho(inicio, inicio + t.len() as u32));
                    verbetes.len() as i32 - 1
                })
            }
        };
        let (ids, mascara) = self.textos.get_or_insert_with(|| (
            vec![0i32; CHUNK_SIZE].into_boxed_slice(), vec![0u8; CHUNK_SIZE >> 3].into_boxed_slice()));
        ids[r] = id;
        mascara[r >> 3] |= bit;
    }

    fn terminar(self, col: usize) -> BlocoColuna {
        BlocoColuna {
            col: col as u32,
            numeros: self.numeros,
            textos: self.textos.map(|(ids, mascara)| (ids, mascara, self.verbetes)),
            avulsas: self.avulsas,
        }
    }
}

/// As colunas `de..ate` do bloco, percorrendo o texto na ordem em que está
/// guardado (linha a linha): coluna a coluna, cada campo lido ficava longe
/// do anterior na memória e o processador esperava a RAM (6,1 s em 70
/// milhões de células, 16/10/2026).
fn montar_grupo(de: usize, ate: usize, bloco: &BlocoLinhas, texto: &str) -> Vec<BlocoColuna> {
    let mut construtores: Vec<Construtor> = (de..ate).map(|_| Construtor::novo()).collect();
    for r in 0..bloco.quantas_linhas() {
        let primeiro = bloco.linhas[r] as usize;
        let campos = bloco.linhas[r + 1] as usize - primeiro;
        for c in de..ate.min(campos) {
            let (ini, fim) = bloco.campo(primeiro + c);
            construtores[c - de].campo(r, texto, ini, fim);
        }
    }
    construtores.into_iter().enumerate().map(|(i, k)| k.terminar(de + i)).collect()
}

/// Classifica as colunas do bloco em `nucleos` threads (cada uma com a sua
/// faixa de colunas); com um núcleo, aqui mesmo.
fn montar_colunas(bloco: &BlocoLinhas, texto: &str, nucleos: usize) -> Vec<BlocoColuna> {
    let colunas = (0..bloco.quantas_linhas())
        .map(|r| (bloco.linhas[r + 1] - bloco.linhas[r]) as usize).max().unwrap_or(0);
    if nucleos <= 1 || colunas <= 1 {
        return montar_grupo(0, colunas, bloco, texto);
    }
    let por_thread = colunas.div_ceil(nucleos.min(colunas));
    std::thread::scope(|escopo| {
        let tarefas: Vec<_> = (0..colunas).step_by(por_thread).map(|de| escopo.spawn(move || {
            montar_grupo(de, (de + por_thread).min(colunas), bloco, texto)
        })).collect();
        tarefas.into_iter().flat_map(|t| t.join().expect("classificar colunas")).collect()
    })
}

/// Lê `leitor` (o arquivo inteiro) para dentro de `aba`, vazia. `delim`
/// já escolhido (ASCII). Devolve as células gravadas.
pub(crate) fn importar(leitor: impl BufRead + Send, delim: u8, aba: &mut Sheet) -> Result<u64, String> {
    use std::time::{Duration, Instant};
    let nucleos = feathersheet_core::paralelismo::nucleos();
    let (para_montar, a_montar) = std::sync::mpsc::sync_channel::<BlocoLinhas>(2);
    let (para_gravar, a_gravar) = std::sync::mpsc::sync_channel::<(BlocoLinhas, Vec<BlocoColuna>)>(2);
    std::thread::scope(|escopo| {
        // 1) dividir o texto em blocos de linhas.
        let divisor = escopo.spawn(move || -> std::io::Result<(bool, Duration)> {
            let mut gasto = Duration::ZERO;
            let mut leitor = leitor;
            let mut divisor = Divisor::novo(delim);
            let mut entregar = |bloco: BlocoLinhas| { let _ = para_montar.send(bloco); };
            loop {
                let buf = leitor.fill_buf()?;
                if buf.is_empty() {
                    break;
                }
                let n = buf.len();
                let t0 = Instant::now();
                divisor.alimentar(buf, &mut entregar);
                gasto += t0.elapsed();
                leitor.consume(n);
                if divisor.excedeu {
                    break;
                }
            }
            Ok((divisor.terminar(&mut entregar), gasto))
        });
        // 2) classificar as colunas de cada bloco.
        let montador = escopo.spawn(move || -> Result<Duration, String> {
            let mut gasto = Duration::ZERO;
            for bloco in a_montar {
                let t0 = Instant::now();
                let texto = std::str::from_utf8(&bloco.texto).map_err(|e| e.to_string())?;
                let colunas = montar_colunas(&bloco, texto, nucleos);
                gasto += t0.elapsed();
                if para_gravar.send((bloco, colunas)).is_err() {
                    break;
                }
            }
            Ok(gasto)
        });
        // 3) gravar na aba (esta thread, dona dela).
        let mut celulas = 0u64;
        let mut padroes = Padroes::default();
        let mut sobras = Vec::new();
        let mut t_gravar = Duration::ZERO;
        for (bloco, colunas) in a_gravar {
            let t0 = Instant::now();
            // Já validado na montagem.
            let texto = std::str::from_utf8(&bloco.texto).unwrap_or_default();
            let linha0 = (bloco.indice << CHUNK_SHIFT) as u32;
            for coluna in colunas {
                if let Some((valores, mascara)) = coluna.numeros {
                    celulas += mascara.iter().map(|b| b.count_ones() as u64).sum::<u64>();
                    aba.carregar_bloco_numeros(coluna.col, bloco.indice, valores, mascara);
                }
                if let Some((ids, mascara, verbetes)) = coluna.textos {
                    celulas += mascara.iter().map(|b| b.count_ones() as u64).sum::<u64>();
                    let dict: Vec<&str> = verbetes.iter().map(|v| match *v {
                        Verbete::Trecho(a, b) => &texto[a as usize..b as usize],
                        Verbete::Bool(true) => "VERDADEIRO",
                        Verbete::Bool(false) => "FALSO",
                    }).collect();
                    aba.carregar_bloco_textos(coluna.col, bloco.indice, ids, mascara, &dict);
                }
                for (r, a, b) in coluna.avulsas {
                    let raw = &texto[a as usize..b as usize];
                    let row = linha0 + r;
                    let pronta = CelulaPreparada::preparar(row, coluna.col, raw, (0, raw.len() as u32), 0, None, &mut padroes);
                    let estilo = formato_do_literal(parse_literal(raw.trim()).format).map(|f| {
                        let mut patch = EstiloParcial::new();
                        patch.insert("format".into(), Campo::Texto(f.into()));
                        patch
                    });
                    let idx = estilo.clone().and_then(|e| aba.registrar_parcial(e));
                    aba.carregar_preparada(pronta, raw, idx, estilo.as_ref(), &mut sobras);
                    sobras.clear();
                    celulas += 1;
                }
            }
            t_gravar += t0.elapsed();
        }
        aba.terminar_carga();
        let (excedeu, t_dividir) = divisor.join().map_err(|_| "CSV: falha ao dividir o texto".to_string())?
            .map_err(|e| format!("Não consegui ler CSV/TSV UTF-8: {e}"))?;
        let t_montar = montador.join().map_err(|_| "CSV: falha ao classificar as colunas".to_string())?
            .map_err(|e| format!("Não consegui ler CSV/TSV UTF-8: {e}"))?;
        feathersheet_core::anotar!(
            "[abrir] CSV por colunas: dividir {:.1} s · classificar {:.1} s ({nucleos} núcleos) · gravar {:.1} s (correm juntos)",
            t_dividir.as_secs_f64(), t_montar.as_secs_f64(), t_gravar.as_secs_f64());
        if excedeu {
            return Err("CSV/TSV excede os limites de linhas ou colunas da planilha".into());
        }
        Ok(celulas)
    })
}

/// Escolhe o separador como o `feather_io::csv_lotes` (numa amostra de
/// 64 KiB) e devolve o leitor com a amostra de volta na frente, sempre em
/// UTF-8: UTF-16 com BOM (o "Texto Unicode" do Excel) e Windows-1252 (o
/// "ANSI" do Excel e dos dados do governo, como os do TSE) são convertidos ao ler.
// shortcut: só a amostra decide a codificação; byte inválido depois dela ainda dá erro.
pub(crate) fn separador_e_leitor<R: BufRead + Send + 'static>(mut leitor: R, tsv: bool, preferido: Option<char>)
    -> std::io::Result<(char, Box<dyn BufRead + Send>)> {
    let mut amostra = Vec::new();
    leitor.by_ref().take(64 * 1024).read_to_end(&mut amostra)?;
    let cod = Codificacao::da_amostra(&amostra, amostra.len() == 64 * 1024);
    let delim = if tsv {
        '\t'
    } else {
        let (texto, _) = cod.converter(&amostra);
        feather_io::csv_lotes::escolher_separador(&texto, preferido)
    };
    let junto = std::io::Cursor::new(amostra).chain(leitor);
    Ok((delim, match cod {
        Codificacao::Utf8 => Box::new(std::io::BufReader::with_capacity(1 << 20, junto)),
        _ => Box::new(Convertido { dentro: junto, cod, resto: Vec::new(), saida: Vec::new(), pos: 0 }),
    }))
}

/// Texto inteiro em memória para UTF-8, pelas mesmas regras do leitor.
pub(crate) fn decodificar(bytes: Vec<u8>) -> String {
    match Codificacao::da_amostra(&bytes, false) {
        Codificacao::Utf8 => String::from_utf8(bytes).unwrap(),
        cod => cod.converter(&bytes).0,
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
enum Codificacao {
    Utf8,
    Utf16 { le: bool },
    Windows1252,
}

/// Os 32 caracteres que o Windows-1252 põe em 0x80–0x9F; o resto é Latin-1.
const CP1252_80: [char; 32] = [
    '€', '\u{81}', '‚', 'ƒ', '„', '…', '†', '‡', 'ˆ', '‰', 'Š', '‹', 'Œ', '\u{8d}', 'Ž', '\u{8f}',
    '\u{90}', '‘', '’', '“', '”', '•', '–', '—', '˜', '™', 'š', '›', 'œ', '\u{9d}', 'ž', 'Ÿ',
];

impl Codificacao {
    /// `cortada`: a amostra não é o arquivo inteiro, então uma sequência
    /// UTF-8 incompleta no fim pode ser só o corte.
    fn da_amostra(amostra: &[u8], cortada: bool) -> Self {
        if amostra.starts_with(b"\xFF\xFE") {
            return Self::Utf16 { le: true };
        }
        if amostra.starts_with(b"\xFE\xFF") {
            return Self::Utf16 { le: false };
        }
        match std::str::from_utf8(amostra) {
            Ok(_) => Self::Utf8,
            Err(e) if e.error_len().is_none() && cortada => Self::Utf8,
            Err(_) => Self::Windows1252,
        }
    }

    /// Converte o que der de `bytes` e diz quantos bytes usou: no UTF-16 um
    /// byte ímpar ou meio par substituto no fim fica para o pedaço seguinte.
    fn converter(self, bytes: &[u8]) -> (String, usize) {
        match self {
            Self::Utf8 => {
                let fim = std::str::from_utf8(bytes).map_or_else(|e| e.valid_up_to(), |s| s.len());
                (std::str::from_utf8(&bytes[..fim]).unwrap().to_owned(), fim)
            }
            Self::Windows1252 => (bytes.iter().map(|&b| match b {
                0x80..=0x9F => CP1252_80[(b - 0x80) as usize],
                _ => b as char,
            }).collect(), bytes.len()),
            Self::Utf16 { le } => {
                let mut unidades: Vec<u16> = bytes.chunks_exact(2)
                    .map(|p| if le { u16::from_le_bytes([p[0], p[1]]) } else { u16::from_be_bytes([p[0], p[1]]) })
                    .collect();
                if unidades.last().is_some_and(|u| (0xD800..0xDC00).contains(u)) {
                    unidades.pop();
                }
                let usados = unidades.len() * 2;
                (char::decode_utf16(unidades).map(|c| c.unwrap_or('\u{FFFD}')).collect(), usados)
            }
        }
    }
}

/// Leitor que entrega em UTF-8 um texto noutra [`Codificacao`].
struct Convertido<R> {
    dentro: R,
    cod: Codificacao,
    /// Bytes do fim do pedaço anterior que ainda não formavam um caractere.
    resto: Vec<u8>,
    saida: Vec<u8>,
    pos: usize,
}

impl<R: Read> Read for Convertido<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.fill_buf()?.len().min(buf.len());
        buf[..n].copy_from_slice(&self.saida[self.pos..self.pos + n]);
        self.consume(n);
        Ok(n)
    }
}

impl<R: Read> BufRead for Convertido<R> {
    fn fill_buf(&mut self) -> std::io::Result<&[u8]> {
        while self.pos >= self.saida.len() {
            let mut cru = std::mem::take(&mut self.resto);
            let antes = cru.len();
            cru.resize(antes + (1 << 20), 0);
            let n = self.dentro.read(&mut cru[antes..])?;
            cru.truncate(antes + n);
            if n == 0 {
                // Fim do arquivo: o que sobrou não completa caractere nenhum.
                self.saida = if cru.is_empty() { Vec::new() } else { "\u{FFFD}".into() };
                self.pos = 0;
                break;
            }
            let (texto, usados) = self.cod.converter(&cru);
            self.resto = cru.split_off(usados);
            self.saida = texto.into_bytes();
            self.pos = 0;
        }
        Ok(&self.saida[self.pos..])
    }

    fn consume(&mut self, n: usize) {
        self.pos += n;
    }
}

#[cfg(test)]
mod testes {
    use feathersheet_core::planilha::sheet::Sheet;
    use feathersheet_core::planilha::workbook::Workbook;

    /// A referência: o divisor do `feather_io` e um `set_cell_raw` por campo;
    /// o que pede formato (data, hora, %) vai como digitado (`set_cell_divisao`).
    fn celula_a_celula(bytes: &[u8], tsv: bool, separador: Option<char>) -> Workbook {
        let mut w = Workbook::new();
        let aba = &mut w.aba_mut(0).unwrap().sheet;
        let mut linha = 0u32;
        let entregar = |lote: &mut Vec<Vec<String>>| {
            for campos in lote.drain(..) {
                for (c, v) in campos.into_iter().enumerate() {
                    if v.is_empty() {
                    } else if !v.starts_with('=') && super::formato_do_literal(super::parse_literal(v.trim()).format).is_some() {
                        aba.set_cell_divisao(linha, c as u32, v.trim());
                    } else {
                        aba.set_cell_raw(linha, c as u32, v, None);
                    }
                }
                linha += 1;
            }
        };
        let leitor = std::io::Cursor::new(bytes.to_vec());
        if tsv {
            feather_io::csv_lotes::dividir_csv_leitor(leitor, Some('\t'), 512, entregar).unwrap();
        } else {
            feather_io::csv_lotes::dividir_csv_leitor_preferindo(leitor, separador, 512, entregar).unwrap();
        }
        w
    }

    fn por_colunas(bytes: &[u8], tsv: bool, separador: Option<char>) -> Workbook {
        let mut w = Workbook::new();
        let aba = &mut w.aba_mut(0).unwrap().sheet;
        let (delim, leitor) = super::separador_e_leitor(std::io::Cursor::new(bytes.to_vec()), tsv, separador).unwrap();
        super::importar(leitor, delim as u8, aba).unwrap();
        w
    }

    fn iguais(a: &Sheet, b: &Sheet, caso: &str) {
        assert_eq!((a.min_row, a.max_row, a.min_col, a.max_col), (b.min_row, b.max_row, b.min_col, b.max_col), "{caso}: limites");
        for r in a.min_row..=a.max_row {
            for c in a.min_col..=a.max_col {
                assert_eq!(a.get_raw(r, c), b.get_raw(r, c), "{caso}: texto em ({r},{c})");
                assert_eq!(a.get_value(r, c), b.get_value(r, c), "{caso}: valor em ({r},{c})");
            }
        }
    }

    const VALORES: &[&str] = &[
        "123", "-4.5", "0.734", "007", "1.234,56", "1,5", "1,234.5", "R$ 10,00", "$5", "-$3", "50%", "-1,5%",
        "2019-05-03", "03/05/2019", "31/02/2020", "12:30", "2019-05-03 12:30", "TRUE", "false", "Verdadeiro",
        "FALSO", "texto", "  espaço  ", "", "   ", "=A1*2", "=SOMA(B1:B3)", "#N/A", "1e5", "1E-3", ".5", "5.",
        "-", "inf", "NaN", "１２", "ação", "x\"y", "1234567890123456789012345678901234567890", "-0", "R$",
        "\"citado, com vírgula\"", "\"linha\nquebrada\"", "\"a\"\"b\"", "\"\"", "\"\"x", "abc\"", "3 maçãs",
    ];

    /// Arquivo com todos os casos, linhas de tamanhos diferentes e as três
    /// quebras de linha, longo o bastante para cruzar blocos de 65.536.
    fn gerar(linhas: usize, sep: char, quebra_variada: bool) -> Vec<u8> {
        let mut s = String::from("\u{feff}id");
        for c in 1..9 {
            s.push(sep);
            s.push_str(&format!("col{c}"));
        }
        s.push('\n');
        let mut semente = 12345u64;
        let mut proximo = || { semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407); (semente >> 33) as usize };
        for r in 0..linhas {
            let colunas = 1 + proximo() % 10;
            for c in 0..colunas {
                if c > 0 { s.push(sep); }
                // Colunas com cara de verdade (só número, só texto repetido) e misturadas.
                match c {
                    0 => s.push_str(&r.to_string()),
                    1 => s.push_str(&format!("{}.{}", proximo() % 1000, proximo() % 100)),
                    2 => s.push_str(["pop", "rock", "jazz", "samba"][proximo() % 4]),
                    _ => s.push_str(VALORES[proximo() % VALORES.len()]),
                }
            }
            s.push_str(if quebra_variada { ["\n", "\r\n", "\r"][proximo() % 3] } else { "\n" });
        }
        s.into_bytes()
    }

    #[test]
    fn por_colunas_igual_a_celula_a_celula() {
        // Passa da fronteira do primeiro bloco (65.536 linhas).
        let csv = gerar(66_000, ',', true);
        iguais(&celula_a_celula(&csv, false, None).aba(0).unwrap().sheet,
            &por_colunas(&csv, false, None).aba(0).unwrap().sheet, "CSV detectado");
        let csv = gerar(3_000, ',', true);
        for separador in [Some(','), Some(';')] {
            let caso = format!("CSV {separador:?}");
            iguais(&celula_a_celula(&csv, false, separador).aba(0).unwrap().sheet,
                &por_colunas(&csv, false, separador).aba(0).unwrap().sheet, &caso);
        }
        let pv = gerar(2_000, ';', false);
        iguais(&celula_a_celula(&pv, false, None).aba(0).unwrap().sheet,
            &por_colunas(&pv, false, None).aba(0).unwrap().sheet, "ponto e vírgula");
        let tsv = gerar(2_000, '\t', true);
        iguais(&celula_a_celula(&tsv, true, None).aba(0).unwrap().sheet,
            &por_colunas(&tsv, true, None).aba(0).unwrap().sheet, "TSV");
    }

    /// Os atalhos de `classificar` dão sempre o que o `parse_literal` daria.
    #[test]
    fn classificar_igual_ao_parse_literal() {
        use feathersheet_core::celulas::input::parse_literal;
        use feathersheet_core::celulas::values::Value;
        let mut casos: Vec<String> = VALORES.iter().map(|v| v.trim().to_string()).filter(|v| !v.is_empty()).collect();
        casos.extend([
            "0", "-0", "0.0", "00.10", "123456789012345", "1234567890123456", "12345678901234567890",
            "0.1234567890123456789012", "0.12345678901234567890123", "9007199254740993", "1.7976931348623157",
            "1e308", "1e309", "-1e-400", "1.5e3", "1E+3", "R$ 5", "r$5,00", "$ 1.000,00", "(R$ 2)", "-R$ -3",
            "10%", "1.000%", "12:30:45", "25:00", "2020-02-29", "2021-02-29", "1/1/20", "01/13/2020",
            "2019-05-03 08:00", "TRUE", "true", "Falso", "verdadeiro", "abc123", "4uLU6hMCjMI75M1A2tKUQC",
            "3e", "e3", "R3", "1r", "1.2.3", "1,2,3", "1 000", "١٢٣", "12\u{a0}", "R$\u{a0}10", "−5", "+5",
            "inf", "-inf", "NaN", "infinity", "0x10", "1_000", ".", "-.", "-", "--1", "1-", "1.", ".1",
        ].map(String::from));
        for t in casos {
            let esperado = parse_literal(&t).value;
            let ok = match (super::classificar(&t), &esperado) {
                (super::Classe::Numero(a), Value::Number(b)) => a.to_bits() == b.to_bits(),
                (super::Classe::Bool(a), Value::Bool(b)) => a == *b,
                (super::Classe::Texto, Value::Text(_)) => true,
                (super::Classe::Outra, v) => parse_literal(&t).format.is_some_and(|f| f != feathersheet_core::celulas::input::NumberFormat::General)
                    || !matches!(v, Value::Number(_) | Value::Bool(_) | Value::Text(_)),
                _ => false,
            };
            assert!(ok, "{t:?}: parse_literal deu {esperado:?}");
        }
        // Números aleatórios com até 17 dígitos e várias casas: o atalho
        // dá o mesmo bit a bit que o parse.
        let mut semente = 99u64;
        for _ in 0..200_000 {
            semente = semente.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
            let inteiro = semente >> (semente % 50 + 8);
            let casas = (semente >> 3) % 18;
            let t = if casas == 0 { inteiro.to_string() } else {
                format!("{}.{:0width$}", inteiro / 1000, (semente >> 11) % 10u64.pow(casas as u32), width = casas as usize)
            };
            assert_eq!(super::numero_simples(t.as_bytes()).map(f64::to_bits), t.parse::<f64>().ok().map(f64::to_bits), "{t}");
        }
    }

    #[test]
    fn pontas_do_arquivo() {
        for texto in ["a,b\n1,2", "a,b\r\n1,2\r\n", "\"x\ny\",1\r", "", "\n\n", "\"aberta,1\n2", "a,\"\"", "\"\"", ",,\n,", "x\r\n\r\ny"] {
            iguais(&celula_a_celula(texto.as_bytes(), false, None).aba(0).unwrap().sheet,
                &por_colunas(texto.as_bytes(), false, None).aba(0).unwrap().sheet, &format!("{texto:?}"));
        }
    }

    #[test]
    fn utf8_invalido_e_linhas_demais_dao_erro() {
        // Byte inválido depois da amostra (que era UTF-8) dá erro na leitura.
        let mut longe = "a,b\n".repeat(20_000).into_bytes();
        longe.extend_from_slice(b"\xff,1\n");
        let mut w = Workbook::new();
        let aba = &mut w.aba_mut(0).unwrap().sheet;
        let (d, l) = super::separador_e_leitor(std::io::Cursor::new(longe), false, None).unwrap();
        assert!(super::importar(l, d as u8, aba).is_err());
        let mut w = Workbook::new();
        let aba = &mut w.aba_mut(0).unwrap().sheet;
        let muitas = "1\n".repeat(feathersheet_core::celulas::refs::MAX_ROWS as usize + 1);
        let (d, l) = super::separador_e_leitor(std::io::Cursor::new(muitas.into_bytes()), false, None).unwrap();
        assert!(super::importar(l, d as u8, aba).is_err());
    }

    #[test]
    fn csv_em_windows_1252_abre_com_acento() {
        // Como os CSVs do TSE: ';' e "ANSI".
        let mut bytes = b"NM_MUNICIPIO;QT_VOTOS\n".to_vec();
        bytes.extend_from_slice(b"S\xc3O JO\xc3O DA BALIZA;12\n\x93BOA VISTA\x94;7\n");
        let w = por_colunas(&bytes, false, None);
        let aba = &w.aba(0).unwrap().sheet;
        assert_eq!(aba.get_raw(1, 0), "SÃO JOÃO DA BALIZA");
        assert_eq!(aba.get_raw(1, 1), "12");
        assert_eq!(aba.get_raw(2, 0), "“BOA VISTA”");
    }

    #[test]
    fn data_e_hora_do_csv_ficam_com_formato() {
        let w = por_colunas("DT;HH;N\n\"27/10/2026\";\"16:32:29\";2026\n".as_bytes(), false, None);
        let aba = &w.aba(0).unwrap().sheet;
        let formato = |c| aba.estilo_mesclado(1, c).and_then(|e| e.get("format").cloned());
        use feathersheet_core::celulas::estilo_parcial::Campo;
        assert_eq!(formato(0), Some(Campo::Texto("date".into())));
        assert_eq!(formato(1), Some(Campo::Texto("time".into())));
        assert_eq!(formato(2), None);
        assert_eq!(aba.get_raw(1, 0), "27/10/2026");
        // E igual ao caminho célula a célula.
        let antes = celula_a_celula("DT;HH\n27/10/2026;16:32:29\n".as_bytes(), false, Some(';'));
        let a = &antes.aba(0).unwrap().sheet;
        assert_eq!(a.estilo_mesclado(1, 0), aba.estilo_mesclado(1, 0));
    }

    #[test]
    fn csv_em_utf16_do_excel_abre() {
        // "Texto Unicode" do Excel: UTF-16LE com BOM, separado por TAB.
        let texto = "\u{feff}Município\tVotos\r\nSão Luiz\t3\r\n𝄞\t4\r\n";
        for le in [true, false] {
            let bytes: Vec<u8> = texto.encode_utf16()
                .flat_map(|u| if le { u.to_le_bytes() } else { u.to_be_bytes() }).collect();
            let w = por_colunas(&bytes, false, None);
            let aba = &w.aba(0).unwrap().sheet;
            assert_eq!(aba.get_raw(0, 0), "Município", "le={le}");
            assert_eq!(aba.get_raw(1, 0), "São Luiz");
            assert_eq!(aba.get_raw(2, 0), "𝄞");
            assert_eq!(aba.get_raw(2, 1), "4");
            assert_eq!(super::decodificar(bytes), texto);
        }
    }

    #[test]
    fn utf16_cortado_no_meio_do_par_continua_no_pedaco_seguinte() {
        let texto: String = "ação 𝄞;".repeat(100_000);
        let mut bytes = vec![0xFF, 0xFE];
        bytes.extend(texto.encode_utf16().flat_map(u16::to_le_bytes));
        let mut leitor = super::Convertido {
            dentro: std::io::Cursor::new(bytes), cod: super::Codificacao::Utf16 { le: true },
            resto: Vec::new(), saida: Vec::new(), pos: 0,
        };
        let mut lido = String::new();
        std::io::Read::read_to_string(&mut leitor, &mut lido).unwrap();
        assert_eq!(lido.trim_start_matches('\u{feff}'), texto);
    }
}

/// Medição (ignorada): cada estágio sozinho, um depois do outro, numa
/// thread. `FEATHERSHEET_CSV=arquivo.csv cargo test --release
/// medir_estagios -- --ignored --nocapture`.
#[cfg(test)]
mod medicao {
    use std::io::BufRead;
    use std::time::Instant;

    #[test]
    #[ignore]
    fn medir_estagios() {
        let caminho = std::env::var("FEATHERSHEET_CSV").expect("FEATHERSHEET_CSV");
        let bytes = std::fs::read(&caminho).unwrap();
        let t = Instant::now();
        let mut blocos = Vec::new();
        let mut divisor = super::Divisor::novo(b',');
        let mut leitor = std::io::BufReader::with_capacity(1 << 20, std::io::Cursor::new(&bytes));
        loop {
            let buf = leitor.fill_buf().unwrap();
            if buf.is_empty() { break; }
            let n = buf.len();
            divisor.alimentar(buf, &mut |b| blocos.push(b));
            leitor.consume(n);
        }
        divisor.terminar(&mut |b| blocos.push(b));
        eprintln!("[dividir] {:.2} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        for b in &blocos { std::str::from_utf8(&b.texto).unwrap(); }
        eprintln!("[utf-8] {:.2} s", t.elapsed().as_secs_f64());
        // Só classificar, sem guardar: quantos de cada e quantos caem no parse_literal.
        let t = Instant::now();
        let (mut num_rapido, mut sem_digito, mut completo, mut vazio) = (0u64, 0u64, 0u64, 0u64);
        for b in &blocos {
            let texto = std::str::from_utf8(&b.texto).unwrap();
            for i in 0..b.fins.len() {
                let (x, y) = b.campo(i);
                let t = texto[x..y].trim();
                if t.is_empty() { vazio += 1; continue; }
                if super::numero_simples(t.as_bytes()).is_some() { num_rapido += 1; }
                else if !t.bytes().any(|c| c.is_ascii_digit()) { sem_digito += 1; }
                else { completo += 1; }
                std::hint::black_box(super::classificar(t));
            }
        }
        eprintln!("[só classificar] {:.2} s · número rápido {num_rapido} · sem dígito {sem_digito} · parse_literal {completo} · vazio {vazio}",
            t.elapsed().as_secs_f64());
        let t = Instant::now();
        let colunas: Vec<_> = blocos.iter().map(|b| super::montar_colunas(b, std::str::from_utf8(&b.texto).unwrap(), 1)).collect();
        eprintln!("[classificar, 1 núcleo] {:.2} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        let mut w = feathersheet_core::planilha::workbook::Workbook::new();
        let aba = &mut w.aba_mut(0).unwrap().sheet;
        for (b, cs) in blocos.iter().zip(colunas) {
            let texto = std::str::from_utf8(&b.texto).unwrap();
            for c in cs {
                if let Some((v, m)) = c.numeros { aba.carregar_bloco_numeros(c.col, b.indice, v, m); }
                if let Some((ids, m, verbetes)) = c.textos {
                    let dict: Vec<&str> = verbetes.iter().map(|v| match *v {
                        super::Verbete::Trecho(a, z) => &texto[a as usize..z as usize],
                        super::Verbete::Bool(true) => "VERDADEIRO",
                        super::Verbete::Bool(false) => "FALSO",
                    }).collect();
                    aba.carregar_bloco_textos(c.col, b.indice, ids, m, &dict);
                }
            }
        }
        eprintln!("[gravar] {:.2} s", t.elapsed().as_secs_f64());
        let t = Instant::now();
        drop(w);
        drop(blocos);
        eprintln!("[soltar] {:.2} s", t.elapsed().as_secs_f64());
    }
}
