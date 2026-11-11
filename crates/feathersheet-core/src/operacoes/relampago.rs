//! Preenchimento relâmpago (Flash Fill do Excel, Ctrl+E).
//!
//! A pessoa digita de 1 a 3 exemplos na coluna ao lado dos dados; daqui sai
//! um *programa* de transformação de texto que explica todos os exemplos, e
//! ele é aplicado às linhas restantes. É uma síntese por exemplos no estilo
//! do FlashFill/PROSE, simplificada:
//!
//! - o programa é uma concatenação de **pedaços**; cada pedaço é uma
//!   constante ou uma subcadeia de uma das colunas de origem, com uma caixa
//!   (igual, MAIÚSCULA, minúscula, Inicial Maiúscula);
//! - as pontas da subcadeia são **posições**: início/fim do texto, índice
//!   absoluto (da esquerda ou da direita), ou antes/depois da k-ésima
//!   ocorrência de um token (espaço, vírgula, @, ponto, hífen, barra,
//!   parênteses, dígitos, maiúsculas, minúsculas, letras, alfanumérico),
//!   contando da esquerda ou da direita; o fim ainda pode ser "n caracteres";
//! - cada forma tem um custo, e vence o programa **mais barato** que
//!   reproduz todos os exemplos. A busca é um Dijkstra no produto das
//!   posições das saídas dos exemplos: um pedaço só é aresta se produz o
//!   trecho seguinte de *todas* as saídas ao mesmo tempo.
//!
//! Ambiguidade: quando dois pedaços empatam em custo no caminho escolhido e
//! dão resultados diferentes numa linha a preencher, nada é escrito — a
//! pessoa precisa dar mais um exemplo. Sem programa, também nada é escrito.

use std::cmp::Reverse;
use std::collections::{BinaryHeap, HashMap};

use crate::planilha::sheet::Sheet;

/// Classe de token que delimita posições.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Token {
    Espaco,
    Virgula,
    Arroba,
    Ponto,
    Hifen,
    Barra,
    AbreParenteses,
    FechaParenteses,
    DoisPontos,
    PontoEVirgula,
    Digitos,
    Maiusculas,
    Minusculas,
    Letras,
    Alfanumerico,
}

const TOKENS: [Token; 15] = [
    Token::Espaco, Token::Virgula, Token::Arroba, Token::Ponto, Token::Hifen,
    Token::Barra, Token::AbreParenteses, Token::FechaParenteses, Token::DoisPontos,
    Token::PontoEVirgula, Token::Digitos, Token::Maiusculas, Token::Minusculas,
    Token::Letras, Token::Alfanumerico,
];

impl Token {
    fn aceita(self, c: char) -> bool {
        match self {
            Token::Espaco => c.is_whitespace(),
            Token::Virgula => c == ',',
            Token::Arroba => c == '@',
            Token::Ponto => c == '.',
            Token::Hifen => c == '-',
            Token::Barra => c == '/',
            Token::AbreParenteses => c == '(',
            Token::FechaParenteses => c == ')',
            Token::DoisPontos => c == ':',
            Token::PontoEVirgula => c == ';',
            Token::Digitos => c.is_ascii_digit(),
            Token::Maiusculas => c.is_uppercase(),
            Token::Minusculas => c.is_lowercase(),
            Token::Letras => c.is_alphabetic(),
            Token::Alfanumerico => c.is_alphanumeric(),
        }
    }

    /// Separadores literais são as âncoras mais naturais; classes largas
    /// (letras, alfanumérico) só vencem quando nada mais explica o exemplo.
    fn custo_classe(self) -> u32 {
        match self {
            Token::Digitos => 1,
            Token::Maiusculas => 2,
            Token::Letras => 3,
            Token::Minusculas => 4,
            Token::Alfanumerico => 5,
            _ => 0,
        }
    }
}

/// As ocorrências (sequências máximas) de um token, em índices de caractere.
fn ocorrencias(s: &[char], t: Token) -> Vec<(usize, usize)> {
    let mut v = Vec::new();
    let mut i = 0;
    while i < s.len() {
        if t.aceita(s[i]) {
            let ini = i;
            while i < s.len() && t.aceita(s[i]) { i += 1; }
            v.push((ini, i));
        } else {
            i += 1;
        }
    }
    v
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Lado {
    /// No começo da ocorrência.
    Antes,
    /// Logo depois da ocorrência.
    Depois,
}

/// Uma posição dentro do texto de origem.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Posicao {
    Inicio,
    Fim,
    /// `k` caracteres a partir da esquerda.
    Absoluta(usize),
    /// `k` caracteres antes do fim.
    DoFim(usize),
    /// Antes/depois da `k`-ésima ocorrência (1 = primeira; -1 = última).
    Token { token: Token, k: i32, lado: Lado },
}

impl Posicao {
    fn avaliar(&self, s: &[char]) -> Option<usize> {
        match *self {
            Posicao::Inicio => Some(0),
            Posicao::Fim => Some(s.len()),
            Posicao::Absoluta(k) => (k <= s.len()).then_some(k),
            Posicao::DoFim(k) => s.len().checked_sub(k),
            Posicao::Token { token, k, lado } => {
                let oc = ocorrencias(s, token);
                let idx = if k > 0 { k as i64 - 1 } else { oc.len() as i64 + k as i64 };
                let (a, b) = *oc.get(usize::try_from(idx).ok()?)?;
                Some(if lado == Lado::Antes { a } else { b })
            }
        }
    }

    fn custo(&self) -> u32 {
        match *self {
            Posicao::Inicio | Posicao::Fim => 0,
            Posicao::Absoluta(_) => 12,
            Posicao::DoFim(_) => 13,
            Posicao::Token { token, k, .. } => custo_k(k) + token.custo_classe(),
        }
    }
}

fn custo_k(k: i32) -> u32 {
    match k {
        1 => 10,
        -1 => 11,
        2 => 14,
        -2 => 15,
        _ => 20 + k.unsigned_abs(),
    }
}

/// Onde termina uma subcadeia.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Termino {
    Posicao(Posicao),
    /// Um número fixo de caracteres depois do início.
    Comprimento(usize),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Caixa {
    Igual,
    Maiuscula,
    Minuscula,
    /// Inicial maiúscula em cada palavra, o resto minúsculo.
    Titulo,
}

impl Caixa {
    fn aplicar(self, s: &[char]) -> String {
        match self {
            Caixa::Igual => s.iter().collect(),
            Caixa::Maiuscula => s.iter().flat_map(|c| c.to_uppercase()).collect(),
            Caixa::Minuscula => s.iter().flat_map(|c| c.to_lowercase()).collect(),
            Caixa::Titulo => {
                let mut saida = String::with_capacity(s.len());
                let mut nova = true;
                for &c in s {
                    if c.is_alphabetic() {
                        if nova { saida.extend(c.to_uppercase()); } else { saida.extend(c.to_lowercase()); }
                        nova = false;
                    } else {
                        saida.push(c);
                        nova = !c.is_alphanumeric();
                    }
                }
                saida
            }
        }
    }

    fn custo(self) -> u32 {
        match self {
            Caixa::Igual => 0,
            Caixa::Maiuscula | Caixa::Minuscula => 3,
            Caixa::Titulo => 4,
        }
    }
}

/// Um pedaço do programa.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Pedaco {
    Constante(String),
    Subcadeia { coluna: usize, inicio: Posicao, fim: Termino, caixa: Caixa },
}

impl Pedaco {
    fn avaliar(&self, entradas: &[Vec<char>]) -> Option<String> {
        match self {
            Pedaco::Constante(c) => Some(c.clone()),
            Pedaco::Subcadeia { coluna, inicio, fim, caixa } => {
                let s = entradas.get(*coluna)?;
                let a = inicio.avaliar(s)?;
                let b = match fim {
                    Termino::Posicao(p) => p.avaliar(s)?,
                    Termino::Comprimento(n) => a + n,
                };
                (a < b && b <= s.len()).then(|| caixa.aplicar(&s[a..b]))
            }
        }
    }

    fn custo(&self) -> u32 {
        match self {
            // Caro por caractere: um texto fixo só entra onde nenhuma subcadeia
            // explica o exemplo (senão "J.S." inteiro venceria "J" + "." + "S").
            Pedaco::Constante(c) => 5 + 40 * c.chars().count() as u32,
            Pedaco::Subcadeia { inicio, fim, caixa, .. } => {
                let pontas = match (inicio, fim) {
                    // A ocorrência inteira de um token ("o primeiro número")
                    // é uma ideia só, não duas posições independentes.
                    (Posicao::Token { token: t1, k: k1, lado: Lado::Antes },
                     Termino::Posicao(Posicao::Token { token: t2, k: k2, lado: Lado::Depois }))
                        if t1 == t2 && k1 == k2 => inicio.custo() + 2,
                    (_, Termino::Posicao(p)) => inicio.custo() + p.custo(),
                    (_, Termino::Comprimento(n)) => inicio.custo() + if *n == 1 { 6 } else { 16 },
                };
                5 + pontas + caixa.custo()
            }
        }
    }
}

/// O programa sintetizado: a concatenação dos pedaços.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Programa {
    pub pedacos: Vec<Pedaco>,
}

impl Programa {
    /// Aplica a uma linha (um texto por coluna de origem). `None` quando
    /// alguma posição não existe nesta linha.
    pub fn aplicar(&self, entradas: &[String]) -> Option<String> {
        let chars: Vec<Vec<char>> = entradas.iter().map(|s| s.chars().collect()).collect();
        self.aplicar_chars(&chars)
    }

    fn aplicar_chars(&self, entradas: &[Vec<char>]) -> Option<String> {
        let mut saida = String::new();
        for p in &self.pedacos {
            saida.push_str(&p.avaliar(entradas)?);
        }
        Some(saida)
    }

    pub fn custo(&self) -> u32 {
        self.pedacos.iter().map(Pedaco::custo).sum()
    }
}

/// Um exemplo: os textos das colunas de origem e a saída esperada.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Exemplo {
    pub entradas: Vec<String>,
    pub saida: String,
}

/// Por que o preenchimento não aconteceu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ErroRelampago {
    /// Nenhuma coluna com dados ao lado da coluna ativa.
    SemDados,
    /// A coluna ainda não tem exemplo digitado.
    SemExemplo,
    /// Nenhum programa explica todos os exemplos.
    SemSolucao,
    /// Mais de um programa igualmente simples, com resultados diferentes.
    Ambiguo,
    /// Todas as linhas com dados já estão preenchidas.
    NadaAPreencher,
}

impl ErroRelampago {
    pub fn mensagem(&self) -> String {
        crate::tr!(match self {
            ErroRelampago::SemDados =>
                "Preenchimento relâmpago: não há dados nas colunas ao lado desta.",
            ErroRelampago::SemExemplo =>
                "Preenchimento relâmpago: digite um exemplo do resultado na primeira linha desta coluna.",
            ErroRelampago::SemSolucao =>
                "Preenchimento relâmpago: nenhum padrão explica os exemplos. Confira os exemplos ou digite outro.",
            ErroRelampago::Ambiguo =>
                "Preenchimento relâmpago: o padrão é ambíguo. Digite mais um exemplo e tente de novo.",
            ErroRelampago::NadaAPreencher =>
                "Preenchimento relâmpago: não há células vazias para preencher nesta coluna.",
        })
    }
}

// ---------------------------------------------------------------- síntese

/// As expressões de posição que caem em `p` no texto `s`, com o custo.
fn posicoes_em(s: &[char], p: usize, tokens: &[(Token, Vec<(usize, usize)>)]) -> Vec<Posicao> {
    let mut v = Vec::new();
    if p == 0 { v.push(Posicao::Inicio); } else { v.push(Posicao::Absoluta(p)); }
    if p == s.len() { v.push(Posicao::Fim); } else { v.push(Posicao::DoFim(s.len() - p)); }
    for (t, oc) in tokens {
        let n = oc.len() as i32;
        for (i, &(a, b)) in oc.iter().enumerate() {
            let k = i as i32 + 1;
            for (pos, lado) in [(a, Lado::Antes), (b, Lado::Depois)] {
                if pos == p {
                    v.push(Posicao::Token { token: *t, k, lado });
                    v.push(Posicao::Token { token: *t, k: k - n - 1, lado });
                }
            }
        }
    }
    v
}

fn caixas_que_levam(origem: &[char], alvo: &[char]) -> Vec<Caixa> {
    let alvo: String = alvo.iter().collect();
    [Caixa::Igual, Caixa::Maiuscula, Caixa::Minuscula, Caixa::Titulo]
        .into_iter()
        .filter(|c| c.aplicar(origem) == alvo)
        .collect()
}

fn mesma_letra(a: char, b: char) -> bool {
    a == b || a.to_lowercase().eq(b.to_lowercase())
}

/// Todos os pedaços de subcadeia que produzem `saida[i..j]` no primeiro
/// exemplo, agrupados por `(i, j)` e ordenados por custo.
fn pedacos_do_exemplo(entradas: &[Vec<char>], saida: &[char]) -> HashMap<(usize, usize), Vec<(u32, Pedaco)>> {
    let mut mapa: HashMap<(usize, usize), Vec<(u32, Pedaco)>> = HashMap::new();
    for (col, s) in entradas.iter().enumerate() {
        let tokens: Vec<(Token, Vec<(usize, usize)>)> =
            TOKENS.iter().map(|&t| (t, ocorrencias(s, t))).filter(|(_, o)| !o.is_empty()).collect();
        let mut cache_pos: HashMap<usize, Vec<Posicao>> = HashMap::new();
        for a in 0..s.len() {
            for i in 0..saida.len() {
                let mut l = 0;
                while a + l < s.len() && i + l < saida.len() && mesma_letra(s[a + l], saida[i + l]) {
                    l += 1;
                    let b = a + l;
                    let caixas = caixas_que_levam(&s[a..b], &saida[i..i + l]);
                    if caixas.is_empty() { continue; }
                    let inicios = cache_pos.entry(a).or_insert_with(|| posicoes_em(s, a, &tokens)).clone();
                    let fins = cache_pos.entry(b).or_insert_with(|| posicoes_em(s, b, &tokens)).clone();
                    let lista = mapa.entry((i, i + l)).or_default();
                    for ini in &inicios {
                        let terminos = fins.iter().map(|f| Termino::Posicao(*f))
                            .chain(std::iter::once(Termino::Comprimento(l)));
                        for fim in terminos {
                            for &caixa in &caixas {
                                let p = Pedaco::Subcadeia { coluna: col, inicio: *ini, fim, caixa };
                                lista.push((p.custo(), p));
                            }
                        }
                    }
                }
            }
        }
    }
    for lista in mapa.values_mut() {
        lista.sort_by_key(|(c, _)| *c);
    }
    mapa
}

/// O caminho achado: cada pedaço com as alternativas de mesmo custo.
struct Caminho {
    pedacos: Vec<Vec<Pedaco>>,
}

/// O Dijkstra no produto das posições de saída de todos os exemplos.
fn buscar(exemplos: &[Exemplo]) -> Option<Caminho> {
    if exemplos.is_empty() { return None; }
    let entradas: Vec<Vec<Vec<char>>> = exemplos.iter()
        .map(|e| e.entradas.iter().map(|s| s.chars().collect()).collect()).collect();
    let saidas: Vec<Vec<char>> = exemplos.iter().map(|e| e.saida.chars().collect()).collect();
    if saidas.iter().any(Vec::is_empty) { return None; }
    let pedacos1 = pedacos_do_exemplo(&entradas[0], &saidas[0]);
    let alvo: Vec<usize> = saidas.iter().map(Vec::len).collect();
    let inicio = vec![0usize; exemplos.len()];

    let mut dist: HashMap<Vec<usize>, u32> = HashMap::new();
    let mut anterior: HashMap<Vec<usize>, (Vec<usize>, Vec<Pedaco>)> = HashMap::new();
    let mut fila = BinaryHeap::new();
    let mut seq: u64 = 0;
    dist.insert(inicio.clone(), 0);
    fila.push(Reverse((0u32, seq, inicio)));

    // Avança os exemplos 2.. com o resultado do pedaço; `None` se algum não casa.
    let avancar = |estado: &[usize], j1: usize, pedaco: &Pedaco| -> Option<Vec<usize>> {
        let mut prox = Vec::with_capacity(estado.len());
        prox.push(j1);
        for m in 1..estado.len() {
            let texto: Vec<char> = pedaco.avaliar(&entradas[m])?.chars().collect();
            let i = estado[m];
            if texto.is_empty() || i + texto.len() > saidas[m].len()
                || saidas[m][i..i + texto.len()] != texto[..] {
                return None;
            }
            prox.push(i + texto.len());
        }
        Some(prox)
    };

    while let Some(Reverse((custo, _, estado))) = fila.pop() {
        if dist.get(&estado).is_some_and(|&d| d < custo) { continue; }
        if estado == alvo {
            let mut pedacos = Vec::new();
            let mut atual = estado;
            while let Some((ant, p)) = anterior.get(&atual) {
                pedacos.push(p.clone());
                atual = ant.clone();
            }
            pedacos.reverse();
            return Some(Caminho { pedacos });
        }
        let i1 = estado[0];
        // Sucessor -> (custo do pedaço, pedaços empatados).
        let mut locais: Vec<(Vec<usize>, u32, Vec<Pedaco>)> = Vec::new();
        let mut indice: HashMap<Vec<usize>, usize> = HashMap::new();
        let mut anotar = |prox: Vec<usize>, c: u32, p: Pedaco| {
            if let Some(&ix) = indice.get(&prox) {
                let item = &mut locais[ix];
                if c < item.1 { item.1 = c; item.2 = vec![p]; }
                else if c == item.1 && item.2.len() < 16 && !item.2.contains(&p) { item.2.push(p); }
            } else {
                indice.insert(prox.clone(), locais.len());
                locais.push((prox, c, vec![p]));
            }
        };
        for j1 in i1 + 1..=alvo[0] {
            let constante = Pedaco::Constante(saidas[0][i1..j1].iter().collect());
            if let Some(prox) = avancar(&estado, j1, &constante) {
                anotar(prox, constante.custo(), constante);
            }
            if let Some(lista) = pedacos1.get(&(i1, j1)) {
                for (c, p) in lista {
                    if let Some(prox) = avancar(&estado, j1, p) {
                        anotar(prox, *c, p.clone());
                    }
                }
            }
        }
        for (prox, c, alternativas) in locais {
            let novo = custo + c;
            if dist.get(&prox).map_or(true, |&d| novo < d) {
                dist.insert(prox.clone(), novo);
                anterior.insert(prox.clone(), (estado.clone(), alternativas));
                seq += 1;
                fila.push(Reverse((novo, seq, prox)));
            }
        }
    }
    None
}

/// Teto de linhas pendentes conferidas na escolha entre pedaços empatados.
const AMOSTRA_DESEMPATE: usize = 10_000;

/// Sintetiza o programa mais simples que explica `exemplos`. Com
/// `pendentes` (as linhas que serão preenchidas), desempata pedaços de mesmo
/// custo pelo que funciona em mais linhas e recusa a ambiguidade.
pub fn sintetizar(exemplos: &[Exemplo], pendentes: &[Vec<String>]) -> Result<Programa, ErroRelampago> {
    let caminho = buscar(exemplos).ok_or(ErroRelampago::SemSolucao)?;
    let amostra: Vec<Vec<Vec<char>>> = pendentes.iter().take(AMOSTRA_DESEMPATE)
        .map(|l| l.iter().map(|s| s.chars().collect()).collect()).collect();
    let mut pedacos = Vec::with_capacity(caminho.pedacos.len());
    for opcoes in caminho.pedacos {
        if opcoes.len() == 1 || amostra.is_empty() {
            pedacos.extend(opcoes.into_iter().next());
            continue;
        }
        let resultados: Vec<Vec<Option<String>>> = opcoes.iter()
            .map(|p| amostra.iter().map(|l| p.avaliar(l)).collect()).collect();
        for linha in 0..amostra.len() {
            let mut visto: Option<&String> = None;
            for r in &resultados {
                if let Some(t) = &r[linha] {
                    match visto {
                        Some(v) if v != t => return Err(ErroRelampago::Ambiguo),
                        _ => visto = Some(t),
                    }
                }
            }
        }
        let melhor = (0..opcoes.len())
            .max_by_key(|&i| (resultados[i].iter().filter(|r| r.is_some()).count(), Reverse(i)))
            .unwrap_or(0);
        pedacos.push(opcoes[melhor].clone());
    }
    Ok(Programa { pedacos })
}

// ------------------------------------------------------------ na planilha

/// Até quantas colunas de cada lado entram como origem.
const MAX_COLUNAS_ORIGEM: u32 = 32;
/// Quantos exemplos a busca usa de saída (os demais só conferem).
const EXEMPLOS_NA_BUSCA: usize = 3;
/// Quantos exemplos a busca aceita ao incluir os que não bateram.
const EXEMPLOS_MAX: usize = 6;

/// O que o preenchimento vai escrever, antes de escrever.
#[derive(Debug, Clone, PartialEq)]
pub struct PlanoRelampago {
    pub programa: Programa,
    /// Colunas de origem, da esquerda para a direita.
    pub origens: Vec<u32>,
    /// Primeira e última linha do bloco de dados.
    pub linhas: (u32, u32),
    /// Linhas usadas como exemplo.
    pub exemplos: Vec<u32>,
    /// (linha, texto) a escrever na coluna alvo.
    pub escrever: Vec<(u32, String)>,
    /// Linhas com dados em que o programa não se aplica (ficam vazias).
    pub sem_padrao: usize,
}

/// Planeja o preenchimento da coluna `coluna` a partir da célula `linha`,
/// lendo cada célula como texto exibido por `ler`. O bloco é a faixa
/// contínua de linhas com algo nas colunas vizinhas (ou na própria coluna):
/// para na primeira linha vazia, como a região atual do Excel.
pub fn planejar(ler: &dyn Fn(u32, u32) -> String, linha: u32, coluna: u32) -> Result<PlanoRelampago, ErroRelampago> {
    use crate::celulas::refs::{MAX_COLS, MAX_ROWS};
    let vizinhas: Vec<u32> = [coluna.checked_sub(1), Some(coluna), (coluna + 1 < MAX_COLS).then_some(coluna + 1)]
        .into_iter().flatten().collect();
    let tem_algo = |r: u32| vizinhas.iter().any(|&c| !ler(r, c).is_empty());
    if !tem_algo(linha) && !(linha > 0 && tem_algo(linha - 1)) {
        return Err(ErroRelampago::SemDados);
    }
    let mut r0 = linha;
    while r0 > 0 && tem_algo(r0 - 1) { r0 -= 1; }
    let mut r1 = linha;
    while r1 + 1 < MAX_ROWS && tem_algo(r1 + 1) { r1 += 1; }
    // Célula ativa logo abaixo do bloco: o bloco é o de cima.
    if !tem_algo(linha) { r1 = linha - 1; }

    let coluna_tem = |c: u32| (r0..=r1).any(|r| !ler(r, c).is_empty());
    let mut esquerda = Vec::new();
    let mut c = coluna;
    while c > 0 && esquerda.len() < MAX_COLUNAS_ORIGEM as usize && coluna_tem(c - 1) {
        c -= 1;
        esquerda.push(c);
    }
    esquerda.reverse();
    let mut direita = Vec::new();
    let mut c = coluna;
    while c + 1 < MAX_COLS && direita.len() < MAX_COLUNAS_ORIGEM as usize && coluna_tem(c + 1) {
        c += 1;
        direita.push(c);
    }
    let origens: Vec<u32> = esquerda.into_iter().chain(direita).collect();
    if origens.is_empty() { return Err(ErroRelampago::SemDados); }

    let mut exemplos = Vec::new();
    let mut pendentes = Vec::new();
    for r in r0..=r1 {
        let entradas: Vec<String> = origens.iter().map(|&c| ler(r, c)).collect();
        if entradas.iter().all(String::is_empty) { continue; }
        let alvo = ler(r, coluna);
        if alvo.is_empty() {
            pendentes.push((r, entradas));
        } else {
            exemplos.push((r, Exemplo { entradas, saida: alvo }));
        }
    }
    if exemplos.is_empty() { return Err(ErroRelampago::SemExemplo); }
    if pendentes.is_empty() { return Err(ErroRelampago::NadaAPreencher); }

    let linhas_pendentes: Vec<Vec<String>> = pendentes.iter().map(|(_, e)| e.clone()).collect();
    let resolver = |lista: &[(u32, Exemplo)]| -> Result<(Programa, Vec<u32>), ErroRelampago> {
        let mut usados: Vec<usize> = (0..lista.len().min(EXEMPLOS_NA_BUSCA)).collect();
        loop {
            let busca: Vec<Exemplo> = usados.iter().map(|&i| lista[i].1.clone()).collect();
            let programa = sintetizar(&busca, &linhas_pendentes)?;
            let falha = (0..lista.len()).find(|i| !usados.contains(i)
                && programa.aplicar(&lista[*i].1.entradas).as_deref() != Some(lista[*i].1.saida.as_str()));
            match falha {
                None => return Ok((programa, lista.iter().map(|(r, _)| *r).collect())),
                Some(i) if usados.len() < EXEMPLOS_MAX => usados.push(i),
                Some(_) => return Err(ErroRelampago::SemSolucao),
            }
        }
    };
    let resultado = match resolver(&exemplos) {
        // A primeira linha do bloco pode ser o cabeçalho ("Nome" | "Primeiro").
        Err(ErroRelampago::SemSolucao) if exemplos.len() > 1 && exemplos[0].0 == r0 =>
            resolver(&exemplos[1..]),
        outro => outro,
    };
    let (programa, linhas_exemplo) = resultado?;

    let mut escrever = Vec::new();
    let mut sem_padrao = 0;
    for (r, entradas) in &pendentes {
        match programa.aplicar(entradas) {
            Some(t) if !t.is_empty() => escrever.push((*r, t)),
            _ => sem_padrao += 1,
        }
    }
    if escrever.is_empty() { return Err(ErroRelampago::SemSolucao); }
    Ok(PlanoRelampago { programa, origens, linhas: (r0, r1), exemplos: linhas_exemplo, escrever, sem_padrao })
}

/// O texto da célula como a pessoa o vê ao editar: o que foi digitado (a
/// data como data, o número sem separador de milhar); de fórmula, o
/// resultado, com o formato da célula quando ela tem um escolhido.
pub fn texto_exibido(sheet: &Sheet, linha: u32, coluna: u32) -> String {
    let cru = sheet.get_raw(linha, coluna);
    if !cru.starts_with('=') { return cru; }
    let valor = sheet.get_value(linha, coluna);
    let estilo = sheet.estilo_resolvido(linha, coluna);
    let formatado = estilo.as_ref().is_some_and(|e|
        e.code.is_some() || e.format != crate::celulas::style::NumberFormat::Auto);
    if formatado {
        crate::planilha::bloco::texto_formatado(&valor, estilo.as_ref())
    } else {
        crate::celulas::values::to_text(&valor)
    }
}

/// Escreve o plano na coluna, numa transação (um Ctrl+Z desfaz tudo).
pub fn aplicar_plano(sheet: &mut Sheet, coluna: u32, plano: &PlanoRelampago) {
    sheet.abrir_transacao();
    for (r, t) in &plano.escrever {
        sheet.set_cell(*r, coluna, t);
    }
    sheet.fechar_transacao();
}

impl Sheet {
    /// Preenchimento relâmpago na coluna da célula (`linha`, `coluna`).
    /// Só escreve nas células vazias da coluna; devolve o plano aplicado.
    pub fn preencher_relampago(&mut self, linha: u32, coluna: u32) -> Result<PlanoRelampago, ErroRelampago> {
        let plano = {
            let ler = |r: u32, c: u32| texto_exibido(self, r, c);
            planejar(&ler, linha, coluna)?
        };
        aplicar_plano(self, coluna, &plano);
        Ok(plano)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::values::Value;

    fn ex(entradas: &[&str], saida: &str) -> Exemplo {
        Exemplo { entradas: entradas.iter().map(|s| s.to_string()).collect(), saida: saida.into() }
    }

    fn linhas(v: &[&[&str]]) -> Vec<Vec<String>> {
        v.iter().map(|l| l.iter().map(|s| s.to_string()).collect()).collect()
    }

    /// Sintetiza com os exemplos e aplica às linhas; `None` onde não se aplica.
    fn rodar(exemplos: &[Exemplo], resto: &[&[&str]]) -> Vec<Option<String>> {
        let pend = linhas(resto);
        let p = sintetizar(exemplos, &pend).expect("programa");
        pend.iter().map(|l| p.aplicar(l)).collect()
    }

    fn todos(v: Vec<Option<String>>) -> Vec<String> {
        v.into_iter().map(|o| o.expect("linha sem padrão")).collect()
    }

    #[test]
    fn separa_nome_e_sobrenome() {
        let resto: &[&[&str]] = &[&["Maria Souza"], &["Pedro Alves"], &["Ana Lima"]];
        assert_eq!(todos(rodar(&[ex(&["João Silva"], "João")], resto)), ["Maria", "Pedro", "Ana"]);
        assert_eq!(todos(rodar(&[ex(&["João Silva"], "Silva")], resto)), ["Souza", "Alves", "Lima"]);
    }

    #[test]
    fn junta_nome_e_sobrenome_de_duas_colunas() {
        let r = rodar(&[ex(&["João", "Silva"], "João Silva")],
            &[&["Maria", "Souza"], &["Pedro", "de Alcântara"]]);
        assert_eq!(todos(r), ["Maria Souza", "Pedro de Alcântara"]);
    }

    #[test]
    fn iniciais_com_ponto_e_caixa() {
        let r = rodar(&[ex(&["João Silva"], "J.S.")], &[&["Maria Souza"], &["pedro alves"]]);
        assert_eq!(r[0].as_deref(), Some("M.S."));
        // Um exemplo em minúsculas pede a caixa alta.
        let r = rodar(&[ex(&["joão silva"], "JS")], &[&["maria souza"], &["Pedro Alves"]]);
        assert_eq!(todos(r), ["MS", "PA"]);
    }

    #[test]
    fn dominio_do_email() {
        let r = rodar(&[ex(&["ana.souza@gmail.com"], "gmail.com")],
            &[&["joao@empresa.com.br"], &["x.y.z@uol.com"]]);
        assert_eq!(todos(r), ["empresa.com.br", "uol.com"]);
    }

    #[test]
    fn extrai_numero_do_texto() {
        let r = rodar(&[ex(&["Pedido 123 entregue"], "123")],
            &[&["Pedido nº 4567 em rota"], &["Caixa 89"], &["Sem número"]]);
        assert_eq!(r[0].as_deref(), Some("4567"));
        assert_eq!(r[1].as_deref(), Some("89"));
        assert_eq!(r[2], None);
    }

    #[test]
    fn reformata_data() {
        let r = rodar(&[ex(&["2024-03-15"], "15/03/2024")], &[&["1999-12-01"], &["2026-10-15"]]);
        assert_eq!(todos(r), ["01/12/1999", "15/10/2026"]);
    }

    #[test]
    fn reformata_telefone() {
        let r = rodar(&[ex(&["27999991234"], "(27) 99999-1234")], &[&["11987654321"]]);
        assert_eq!(todos(r), ["(11) 98765-4321"]);
        let r = rodar(&[ex(&["27 99999 1234"], "27-99999-1234")], &[&["11 3333 4444"]]);
        assert_eq!(todos(r), ["11-3333-4444"]);
    }

    #[test]
    fn sobrenome_virgula_nome_vira_nome_sobrenome() {
        let r = rodar(&[ex(&["Silva, João"], "João Silva")],
            &[&["Souza, Maria"], &["Alcântara, Pedro"]]);
        assert_eq!(todos(r), ["Maria Souza", "Pedro Alcântara"]);
    }

    #[test]
    fn caixa_de_titulo() {
        let r = rodar(&[ex(&["JOÃO DA SILVA"], "João Da Silva")], &[&["maria souza"]]);
        assert_eq!(todos(r), ["Maria Souza"]);
    }

    #[test]
    fn dois_exemplos_desfazem_a_duvida() {
        // Com um exemplo, "Ana" pode ser a primeira palavra; o segundo
        // exemplo mostra que é a palavra do meio.
        let r = rodar(&[ex(&["Maria Ana Souza"], "Ana"), ex(&["Paulo Rui Lima"], "Rui")],
            &[&["Carla Bia Reis"]]);
        assert_eq!(todos(r), ["Bia"]);
    }

    #[test]
    fn sem_solucao_e_ambiguidade_nao_preenchem() {
        assert_eq!(sintetizar(&[ex(&["abc"], "abc"), ex(&["abc"], "xyz")], &[]),
            Err(ErroRelampago::SemSolucao));
        // As duas colunas têm o mesmo texto no exemplo: copiar a primeira ou
        // a segunda custa igual, e as linhas pendentes divergem.
        assert_eq!(sintetizar(&[ex(&["Ana", "Ana"], "Ana")], &linhas(&[&["Bia", "Eva"]])),
            Err(ErroRelampago::Ambiguo));
        // Se as linhas pendentes não divergem, não há dúvida.
        assert!(sintetizar(&[ex(&["Ana", "Ana"], "Ana")], &linhas(&[&["Bia", "Bia"]])).is_ok());
    }

    #[test]
    fn na_planilha_preenche_so_vazias_ate_o_fim_dos_dados_com_um_desfazer() {
        let mut s = Sheet::new("Plan1");
        s.set_cell(0, 0, "Nome completo");
        s.set_cell(0, 1, "Primeiro nome");
        for (i, n) in ["João Silva", "Maria Souza", "Pedro Alves", "Ana Lima"].iter().enumerate() {
            s.set_cell(i as u32 + 1, 0, n);
        }
        s.set_cell(1, 1, "João");
        s.set_cell(3, 1, "Pedro"); // já preenchida: fica como está
        // Linha 7 separada por uma vazia: fora do bloco.
        s.set_cell(6, 0, "Fora Bloco");
        let antes = s.profundidade_do_historico();
        let plano = s.preencher_relampago(2, 1).expect("preencheu");
        assert_eq!(plano.origens, vec![0]);
        assert_eq!(plano.linhas, (0, 4));
        assert_eq!(plano.escrever.len(), 2);
        assert_eq!(s.get_raw(2, 1), "Maria");
        assert_eq!(s.get_raw(3, 1), "Pedro");
        assert_eq!(s.get_raw(4, 1), "Ana");
        assert_eq!(s.get_raw(6, 1), "");
        assert_eq!(s.profundidade_do_historico(), antes + 1);
        assert!(s.undo());
        assert_eq!(s.get_raw(2, 1), "");
        assert_eq!(s.get_raw(4, 1), "");
        assert_eq!(s.get_raw(1, 1), "João");
    }

    #[test]
    fn na_planilha_cabecalho_nao_atrapalha_e_data_vale_pelo_texto_exibido() {
        let mut s = Sheet::new("Plan1");
        // Cabeçalho na primeira linha, também preenchido na coluna alvo.
        s.set_cell(0, 0, "Data");
        s.set_cell(0, 1, "Ano");
        s.set_cell(1, 0, "15/03/2024");
        s.set_cell(2, 0, "01/12/1999");
        s.set_cell(3, 0, "07/07/2007");
        s.set_cell(1, 1, "2024");
        let plano = s.preencher_relampago(1, 1).expect("preencheu");
        assert_eq!(plano.exemplos, vec![1]);
        assert_eq!(s.get_value(2, 1), Value::Number(1999.0));
        assert_eq!(s.get_value(3, 1), Value::Number(2007.0));
    }

    #[test]
    fn texto_longo_sintetiza_rapido() {
        let longo = "Rua das Palmeiras, 1234 - Bairro Jardim da Penha - Vitória/ES - CEP 29060-000";
        let inicio = std::time::Instant::now();
        let r = rodar(&[ex(&[longo], "29060-000")],
            &[&["Av. Central, 10 - Centro - Serra/ES - CEP 29160-123"]]);
        assert_eq!(todos(r), ["29160-123"]);
        assert!(inicio.elapsed().as_secs_f64() < 5.0, "síntese lenta: {:?}", inicio.elapsed());
    }

    #[test]
    fn na_planilha_avisa_sem_exemplo_e_sem_dados() {
        let mut s = Sheet::new("Plan1");
        assert_eq!(s.preencher_relampago(0, 1), Err(ErroRelampago::SemDados));
        s.set_cell(0, 0, "João Silva");
        s.set_cell(1, 0, "Maria Souza");
        assert_eq!(s.preencher_relampago(0, 1), Err(ErroRelampago::SemExemplo));
        s.set_cell(0, 1, "abc");
        s.set_cell(1, 1, "xyz");
        assert_eq!(s.preencher_relampago(0, 1), Err(ErroRelampago::NadaAPreencher));
    }
}
