//! O layout da tabela dinâmica: a árvore de grupos, os subtotais, o
//! recolher/expandir e os filtros — como no Excel (pedido de 24/09/2026).
//!
//! Funcionalidade nova, só do Rust. O [`crate::dinamica::pivot`] junta as linhas numa
//! chave só (`a | b`) e devolve uma matriz plana; aqui cada campo de Linhas é
//! um NÍVEL, e o resultado vem com um mapa de cada linha (cabeçalho, grupo,
//! item, subtotal, total geral; o nível; os botões +/−), que é o que a grade
//! usa para desenhar a dinâmica e saber onde o clique recolhe.
//!
//! Dois layouts:
//!
//! - **Compacto** (o padrão do Excel): uma coluna de rótulos. O grupo é uma
//!   linha com o subtotal dele, e os itens vêm embaixo, um nível mais fundo
//!   (a grade recua). `a` com `b 3` e `c 5` recolhido vira `a 8`.
//! - **Tabular**: uma coluna por campo. O rótulo de fora se repete em cada
//!   linha ou fica em branco depois da primeira (`repetir`), e cada grupo de
//!   fora termina numa linha "a Total" (`subtotais`).
//!
//! A configuração antiga — tabular, rótulo repetido, sem subtotal, nada
//! recolhido, sem filtro — continua saindo pelo `montar_dinamica_com` de
//! sempre, igual até o último byte: as dinâmicas que já existem não mudam de
//! cara sozinhas.

use std::cmp::Ordering;
use std::collections::{HashMap, HashSet};

use crate::dinamica::pivot::{chave_de, montar_dinamica_com_chaves, montar_dinamica_filtrada, parse_date_key, DinamicaConfig};
use crate::dinamica::acumulador::{Acumulador, Medida};
use crate::dinamica::campo_calculado::{self, Expr};
use crate::dinamica::exibicao::{
    chave_agrupada, comparar_natural, mostrar_como, CampoExtra, Eixo, Extras, Grade, MostrarComo, Ordenacao,
};
use crate::dinamica::pivot::AgruparData;
use crate::celulas::values::{to_text, Matrix, Value};

/// Separa os níveis de um caminho de grupo (`a` → `b`): um caractere que
/// ninguém digita numa célula.
pub const SEPARADOR: char = '\u{1f}';

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Layout {
    Compacto,
    Tabular,
}

/// O que a dinâmica mostra além das contas.
#[derive(Debug, Clone)]
pub struct Opcoes {
    pub layout: Layout,
    /// Tabular: o rótulo de fora aparece em toda linha do grupo.
    pub repetir_rotulos: bool,
    /// Subtotal por grupo: na linha do grupo (compacto) ou numa linha
    /// "a Total" depois dele (tabular).
    pub subtotais: bool,
    /// Compacto com subtotais: a conta numa linha "a Total" embaixo do
    /// grupo em vez de na linha dele (o tabular já é sempre embaixo).
    pub subtotal_embaixo: bool,
    /// Os grupos recolhidos, pelo caminho (níveis unidos por [`SEPARADOR`]).
    pub recolhidos: HashSet<String>,
    /// Filtros de relatório. Coluna fora da lista = "(Tudo)".
    pub filtros: Vec<Filtro>,
}

/// Um filtro de relatório: a coluna da origem (absoluta, como em `linhas`)
/// e os valores que ficam — ou, com `excluir`, os que saem. O "saem" é para
/// quando a lista de valores veio cortada (teto do `valores_distintos`):
/// "ficar só com os marcados" tiraria calado o que nem apareceu na lista.
#[derive(Debug, Clone)]
pub struct Filtro {
    pub coluna: usize,
    pub valores: HashSet<String>,
    pub excluir: bool,
}

impl Opcoes {
    /// O que as dinâmicas faziam antes de existir layout.
    pub fn antigas() -> Self {
        Opcoes {
            layout: Layout::Tabular,
            repetir_rotulos: true,
            subtotais: false,
            subtotal_embaixo: false,
            recolhidos: HashSet::new(),
            filtros: Vec::new(),
        }
    }

    fn sao_antigas(&self) -> bool {
        self.layout == Layout::Tabular
            && self.repetir_rotulos
            && !self.subtotais
            && self.recolhidos.is_empty()
            && self.filtros.is_empty()
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TipoLinha {
    Cabecalho,
    Grupo,
    Item,
    Subtotal,
    Total,
}

/// Um botão +/− numa célula do resultado.
#[derive(Debug, Clone, PartialEq)]
pub struct Marca {
    /// Coluna relativa ao canto do resultado.
    pub coluna: usize,
    pub caminho: String,
    pub recolhido: bool,
}

/// O que é cada linha do resultado, para a grade desenhar.
#[derive(Debug, Clone, PartialEq)]
pub struct InfoLinha {
    pub tipo: TipoLinha,
    /// Profundidade do rótulo (0 = campo de fora). O compacto recua por ela.
    pub nivel: usize,
    pub marcas: Vec<Marca>,
}

impl InfoLinha {
    fn nova(tipo: TipoLinha, nivel: usize) -> Self {
        InfoLinha { tipo, nivel, marcas: Vec::new() }
    }
}

pub struct Resultado {
    pub matriz: Matrix,
    pub linhas: Vec<InfoLinha>,
    /// Quantas colunas, à esquerda, são de rótulo.
    pub colunas_de_rotulo: usize,
    /// De onde vem cada linha do resultado: o caminho dos itens de Linhas
    /// (`""` = todas, no total geral; `None` = cabeçalho). Vazio quando a
    /// dinâmica saiu pelo caminho antigo (ver [`linhas_da_celula`]).
    pub caminhos: Vec<Option<String>>,
    /// O mesmo para cada coluna: a chave dos itens de Colunas (`""` = o
    /// total geral da linha ou sem campo em Colunas; `None` = rótulo).
    pub chaves_colunas: Vec<Option<String>>,
}

/// Um nó da árvore de grupos. `ordenados` é a ordem dos filhos depois de
/// [`ordenar`]: por padrão o texto, como o `BTreeSet` das chaves do
/// [`crate::dinamica::pivot`].
#[derive(Default, Clone)]
struct No {
    filhos: HashMap<String, No>,
    /// Ordem natural do item (faixa, mês...), quando o campo está agrupado.
    ordem: Option<f64>,
    ordenados: Vec<String>,
}

impl No {
    fn filho(&mut self, nome: String, ordem: Option<f64>) -> &mut No {
        let f = self.filhos.entry(nome).or_default();
        f.ordem = ordem;
        f
    }

    fn descer(&self, prefixo: &[String]) -> Option<&No> {
        prefixo.iter().try_fold(self, |no, n| no.filhos.get(n))
    }
}

/// Os números de cada medida.
type Numeros = Vec<Acumulador>;

/// Separa o caminho de Linhas da chave de Colunas na chave de `por_caminho`.
const JUNTA: char = '\u{1e}';

/// As somas do grupo `chave`, criadas na primeira vez: só então a chave é
/// copiada (antes, uma cópia por linha e por nível — 1 milhão de linhas).
fn somas_de<'a>(mapa: &'a mut HashMap<String, Numeros>, chave: &str, config: &DinamicaConfig) -> &'a mut Numeros {
    if !mapa.contains_key(chave) {
        mapa.insert(chave.to_owned(), acumuladores(config));
    }
    mapa.get_mut(chave).expect("acabou de entrar")
}

fn acumuladores(config: &DinamicaConfig) -> Numeros {
    config.valores.iter().map(|m| Acumulador::novo(m.op)).collect()
}

/// Ordena os filhos de `no` (e, descendo, os de baixo) conforme a ordenação
/// do campo de cada nível. `total(caminho, medida)` é o total do item, para
/// "por valor".
fn ordenar(no: &mut No, nivel: usize, caminho: &str, campos: &[CampoExtra], total: &dyn Fn(&str, usize) -> f64) {
    let ord = campos.get(nivel).map(|c| c.ordenacao).unwrap_or_default();
    let mut nomes: Vec<String> = no.filhos.keys().cloned().collect();
    let ordem_de = |n: &String| no.filhos[n].ordem;
    let caminho_de = |n: &str| if caminho.is_empty() { n.to_string() } else { format!("{caminho}{SEPARADOR}{n}") };
    let por_padrao = |a: &String, b: &String| match (ordem_de(a), ordem_de(b)) {
        (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal).then_with(|| a.cmp(b)),
        _ => a.cmp(b),
    };
    match ord {
        Ordenacao::Padrao => nomes.sort_by(por_padrao),
        Ordenacao::Crescente | Ordenacao::Decrescente => {
            nomes.sort_by(|a, b| match (ordem_de(a), ordem_de(b)) {
                (Some(x), Some(y)) => x.partial_cmp(&y).unwrap_or(Ordering::Equal).then_with(|| a.cmp(b)),
                _ => comparar_natural(a, b),
            });
            if ord == Ordenacao::Decrescente {
                nomes.reverse();
            }
        }
        Ordenacao::PorValor { medida, crescente } => {
            nomes.sort_by(|a, b| {
                let (x, y) = (total(&caminho_de(a), medida), total(&caminho_de(b), medida));
                let o = x.partial_cmp(&y).unwrap_or(Ordering::Equal);
                (if crescente { o } else { o.reverse() }).then_with(|| comparar_natural(a, b))
            });
        }
    }
    for n in &nomes {
        let c = caminho_de(n);
        if let Some(f) = no.filhos.get_mut(n) {
            ordenar(f, nivel + 1, &c, campos, total);
        }
    }
    no.ordenados = nomes;
}

/// As chaves das folhas de `no`, em ordem, unidas por [`SEPARADOR`].
fn folhas(no: &No, caminho: &str, saida: &mut Vec<String>) {
    for n in &no.ordenados {
        let c = if caminho.is_empty() { n.clone() } else { format!("{caminho}{SEPARADOR}{n}") };
        if no.filhos[n].filhos.is_empty() {
            saida.push(c);
        } else {
            folhas(&no.filhos[n], &c, saida);
        }
    }
}

/// O item do campo na posição `d` de uma linha: a chave e a ordem natural.
/// [`chave_do_campo`] de um valor já lido: sem agrupamento, o texto vira
/// a chave sem cópia.
fn chave_do_valor(v: Value, d: usize, campos: &[CampoExtra], agrupar_data: Option<AgruparData>) -> (String, Option<f64>) {
    match campos.get(d).and_then(|c| c.agrupamento) {
        Some(_) => chave_do_campo(&v, d, campos, agrupar_data),
        None => (chave_de(v, agrupar_data), None),
    }
}

fn chave_do_campo(
    v: &Value,
    d: usize,
    campos: &[CampoExtra],
    agrupar_data: Option<AgruparData>,
) -> (String, Option<f64>) {
    match campos.get(d).and_then(|c| c.agrupamento) {
        Some(a) => {
            let (k, o) = chave_agrupada(v, &a);
            (k, Some(o))
        }
        None => (parse_date_key(v, agrupar_data), None),
    }
}

/// O rótulo e a ordem do item que `v` vira num campo de Linhas ou de
/// Colunas, com o agrupamento do campo — o mesmo texto que a dinâmica
/// mostra. A interface usa para listar os itens base de "% de" e
/// "Diferença de".
pub fn rotulo_do_item(v: &Value, extra: &CampoExtra, agrupar_data: Option<AgruparData>) -> (String, Option<f64>) {
    chave_do_campo(v, 0, std::slice::from_ref(extra), agrupar_data)
}

/// O formato de número que a coluna `j` do resultado pede pelo "Mostrar
/// valores como" da medida dela (`None` = o formato de sempre). Depois das
/// colunas de rótulo, as de valor vêm uma por medida, em ciclo (cada coluna
/// de Colunas e o total geral da linha).
pub fn formato_da_coluna(resultado: &Resultado, extras: &Extras, n_medidas: usize, j: usize) -> Option<&'static str> {
    if n_medidas == 0 || j < resultado.colunas_de_rotulo {
        return None;
    }
    extras.mostrar_como((j - resultado.colunas_de_rotulo) % n_medidas).formato_sugerido()
}

/// Monta a dinâmica com o layout pedido. `valor(linha, coluna)` é relativo à
/// origem, e a linha 0 é o cabeçalho — o mesmo contrato do
/// [`crate::dinamica::pivot::montar_dinamica_com`].
pub fn montar(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    opcoes: &Opcoes,
) -> Resultado {
    montar_com_extras(total_linhas, valor, config, opcoes, &Extras::default())
}

/// [`montar`] com os [`Extras`]: mostrar valores como, agrupar em faixas ou
/// partes de data, classificar itens e totais gerais. Com `Extras::default()`
/// sai exatamente o que `montar` sempre deu.
pub fn montar_com_extras(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    opcoes: &Opcoes,
    extras: &Extras,
) -> Resultado {
    let n = config.linhas.len();
    let padrao = extras.eh_padrao();
    // Mesmo sem dimensões de linha, filtros de relatório são aplicados
    // durante a varredura, sem armazenar os índices dos milhões de registros.
    if padrao && n == 0 && !opcoes.filtros.is_empty() {
        let c0 = config.origem.c0 as usize;
        let matriz = montar_dinamica_filtrada(total_linhas, &valor, config, |r| {
            opcoes.filtros.iter().all(|f| {
                let chave = parse_date_key(&valor(r, f.coluna.saturating_sub(c0)), None);
                f.valores.contains(&chave) != f.excluir
            })
        });
        let linhas = mapa_antigo(&matriz, 0);
        return Resultado { matriz, linhas, colunas_de_rotulo: 1, caminhos: Vec::new(), chaves_colunas: Vec::new() };
    }
    if total_linhas <= 1 || (padrao && (opcoes.sao_antigas() || n == 0)) {
        let (matriz, chaves_linhas, chaves_cols) = montar_dinamica_com_chaves(total_linhas, &valor, config, |_| true);
        let linhas = mapa_antigo(&matriz, n);
        // Os mesmos caminhos da árvore de grupos (cabeçalho sem, total
        // geral vazio = todas as linhas): sem eles o Detalhes montava tudo
        // de novo pelo caminho estruturado (10 s em 1 milhão de linhas).
        let juntar = |k: &Vec<String>| Some(k.join(&SEPARADOR.to_string()));
        let (caminhos, chaves_colunas) = if matriz.height() < 2 {
            (Vec::new(), Vec::new())
        } else {
            let mut caminhos = vec![None];
            caminhos.extend(chaves_linhas.iter().map(juntar));
            caminhos.push(Some(String::new()));
            let n_medidas = config.valores.len();
            let mut cols: Vec<Option<String>> = vec![None; n.max(1)];
            for k in &chaves_cols {
                cols.extend(std::iter::repeat(juntar(k)).take(n_medidas));
            }
            if !config.colunas.is_empty() {
                cols.extend(std::iter::repeat(Some(String::new())).take(n_medidas));
            }
            (caminhos, cols)
        };
        return Resultado { matriz, linhas, colunas_de_rotulo: n.max(1), caminhos, chaves_colunas };
    }
    montar_estruturado(total_linhas, &valor, config, opcoes, extras)
}

/// As linhas da origem (a partir de 1, como em `valor`) que formam a célula
/// (`linha`, `coluna`) do resultado: o "detalhar" do duplo clique do Excel.
/// Devolve junto o resultado montado pela árvore de grupos, para a interface
/// conferir que é o mesmo que está na planilha (a dinâmica simples sai pelo
/// caminho antigo, que não anota de onde vem cada célula). `None` fora das
/// células de valor.
pub fn linhas_da_celula(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    opcoes: &Opcoes,
    extras: &Extras,
    linha: usize,
    coluna: usize,
) -> Option<(Resultado, Vec<usize>)> {
    // O mesmo cálculo da grade: a dinâmica simples sai pelo caminho rápido,
    // que agora também anota os caminhos (antes, sempre a árvore inteira).
    let resultado = montar_com_extras(total_linhas, &valor, config, opcoes, extras);
    let caminho = resultado.caminhos.get(linha)?.clone()?;
    let ck = resultado.chaves_colunas.get(coluna)?.clone()?;
    let achadas = linhas_do_caminho(total_linhas, &valor, config, opcoes, extras, &caminho, &ck);
    Some((resultado, achadas))
}

/// As linhas da origem de um caminho de Linhas e uma chave de Colunas já
/// conhecidos (de um [`Resultado`] guardado): uma passada só pela origem,
/// sem montar a dinâmica de novo.
pub fn linhas_do_caminho(
    total_linhas: usize,
    valor: impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    opcoes: &Opcoes,
    extras: &Extras,
    caminho: &str,
    ck: &str,
) -> Vec<usize> {
    let partes = |t: &str| -> Vec<String> {
        if t.is_empty() { Vec::new() } else { t.split(SEPARADOR).map(str::to_string).collect() }
    };
    let (alvo_linhas, alvo_colunas) = (partes(caminho), partes(ck));
    let rel = |c: usize| c.saturating_sub(config.origem.c0 as usize);
    let mut achadas = Vec::new();
    'linhas: for r in 1..total_linhas {
        for f in &opcoes.filtros {
            if f.valores.contains(&parse_date_key(&valor(r, rel(f.coluna)), None)) == f.excluir {
                continue 'linhas;
            }
        }
        for (d, alvo) in alvo_linhas.iter().enumerate() {
            let (k, _) = chave_do_campo(&valor(r, rel(config.linhas[d])), d, &extras.linhas, config.agrupar_data);
            let k = extras.juntar(config.linhas[d], k);
            if k != *alvo {
                continue 'linhas;
            }
        }
        for (d, alvo) in alvo_colunas.iter().enumerate() {
            let (k, _) = chave_do_campo(&valor(r, rel(config.colunas[d])), d, &extras.colunas, config.agrupar_data);
            let k = extras.juntar(config.colunas[d], k);
            if k != *alvo {
                continue 'linhas;
            }
        }
        achadas.push(r);
    }
    achadas
}

/// A dinâmica pela árvore de grupos (layouts, subtotais, extras), anotando
/// de onde vem cada linha e cada coluna do resultado.
/// Um item calculado já compilado: o nome e a fórmula sobre `nomes` (os itens do nível).
struct Calculado {
    nome: String,
    expr: Expr,
    nomes: Vec<String>,
}

/// Os itens que aparecem na profundidade `d` da árvore, de todos os pais.
fn nomes_no_nivel(no: &No, d: usize, saida: &mut Vec<String>) {
    for (n, f) in &no.filhos {
        if d == 0 {
            if !saida.contains(n) { saida.push(n.clone()); }
        } else {
            nomes_no_nivel(f, d - 1, saida);
        }
    }
}

/// Os itens calculados de cada nível do eixo (`campos` = as colunas da origem em Linhas
/// ou em Colunas). Fórmula que não compila fica de fora.
fn calculados_do_eixo(raiz: &No, campos: &[usize], extras: &Extras) -> Vec<Vec<Calculado>> {
    if extras.itens_calculados.is_empty() { return Vec::new(); }
    campos.iter().enumerate().map(|(d, &c)| {
        let do_campo: Vec<_> = extras.itens_calculados.iter().filter(|i| i.coluna == c).collect();
        if do_campo.is_empty() { return Vec::new(); }
        let mut nomes = Vec::new();
        nomes_no_nivel(raiz, d, &mut nomes);
        nomes.sort();
        do_campo.into_iter().filter(|i| !nomes.contains(&i.nome)).filter_map(|i| {
            campo_calculado::compilar(&i.formula, &nomes).ok()
                .map(|expr| Calculado { nome: i.nome.clone(), expr, nomes: nomes.clone() })
        }).collect()
    }).collect()
}

fn campos_da_expr(e: &Expr, saida: &mut Vec<usize>) {
    match e {
        Expr::Campo(k) => saida.push(*k),
        Expr::Neg(a) => campos_da_expr(a, saida),
        Expr::Bin(_, a, b) => { campos_da_expr(a, saida); campos_da_expr(b, saida); }
        Expr::Numero(_) => {}
    }
}

/// Junta a estrutura de `b` em `a` (os filhos que faltam), para o item calculado ter
/// embaixo os subitens dos itens que ele usa.
fn juntar_arvore(a: &mut No, b: &No) {
    for n in &b.ordenados {
        match a.filhos.get_mut(n) {
            Some(f) => juntar_arvore(f, &b.filhos[n]),
            None => {
                a.filhos.insert(n.clone(), b.filhos[n].clone());
                a.ordenados.push(n.clone());
            }
        }
    }
}

/// Põe os itens calculados no fim de cada pai do nível deles, onde algum item usado existe.
fn inserir_calculados(no: &mut No, d: usize, niveis: &[Vec<Calculado>]) {
    if let Some(calc) = niveis.get(d).filter(|v| !v.is_empty()) {
        for c in calc {
            let mut usados = Vec::new();
            campos_da_expr(&c.expr, &mut usados);
            let mut novo: Option<No> = None;
            for k in usados {
                if let Some(f) = no.filhos.get(&c.nomes[k]) {
                    juntar_arvore(novo.get_or_insert_with(No::default), f);
                }
            }
            if let Some(novo) = novo.filter(|_| !no.filhos.contains_key(&c.nome)) {
                no.filhos.insert(c.nome.clone(), novo);
                no.ordenados.push(c.nome.clone());
            }
        }
    }
    if d + 1 < niveis.len() {
        let nomes = no.ordenados.clone();
        for n in nomes {
            if let Some(f) = no.filhos.get_mut(&n) { inserir_calculados(f, d + 1, niveis); }
        }
    }
}

/// O valor da chave (caminho de Linhas ou chave de Colunas) com os itens calculados: o
/// primeiro nível que for item calculado vira a conta sobre os irmãos.
fn com_calculados(chave: &str, niveis: &[Vec<Calculado>], base: &dyn Fn(&str) -> f64) -> f64 {
    if chave.is_empty() || niveis.iter().all(Vec::is_empty) {
        return base(chave);
    }
    // Procura sem alocar: a maioria das chaves não tem item calculado.
    let achado = chave.split(SEPARADOR).enumerate()
        .find_map(|(d, s)| niveis.get(d)?.iter().find(|c| c.nome == s).map(|c| (d, c)));
    let Some((d, c)) = achado else { return base(chave) };
    let partes: Vec<&str> = chave.split(SEPARADOR).collect();
    campo_calculado::avaliar(&c.expr, &|k| {
        let mut outra = partes.clone();
        outra[d] = &c.nomes[k];
        com_calculados(&outra.join(&SEPARADOR.to_string()), niveis, base)
    })
}

fn montar_estruturado(
    total_linhas: usize,
    valor: &impl Fn(usize, usize) -> Value,
    config: &DinamicaConfig,
    opcoes: &Opcoes,
    extras: &Extras,
) -> Resultado {
    let n = config.linhas.len();
    let c0 = config.origem.c0 as usize;
    let rel = |c: usize| c.saturating_sub(c0);
    let n_medidas = config.valores.len();
    let cabecalho = |c: usize| {
        let t = to_text(&valor(0, rel(c)));
        if t.trim().is_empty() {
            format!("Coluna {}", c + 1)
        } else {
            t
        }
    };

    // Uma passada: cada linha vai para o seu grupo em TODOS os níveis, nas
    // linhas e nas colunas.
    let mut raiz = No::default();
    let mut raiz_col = No::default();
    // Chave = caminho + JUNTA + chave da coluna: procurar não aloca.
    let mut por_caminho: HashMap<String, Numeros> = HashMap::new();
    let mut dupla = String::new();
    let mut por_caminho_total: HashMap<String, Numeros> = HashMap::new();
    let mut por_coluna: HashMap<String, Numeros> = HashMap::new();
    let mut tudo: Numeros = acumuladores(config);
    let mut medidas: Vec<Medida> = Vec::with_capacity(n_medidas);
    let empurrar = |alvo: &mut Numeros, medidas: &[Medida]| {
        for (acc, &m) in alvo.iter_mut().zip(medidas) { acc.receber_medida(m); }
    };
    let sep = SEPARADOR.to_string();

    'linhas: for r in 1..total_linhas {
        for f in &opcoes.filtros {
            if f.valores.contains(&parse_date_key(&valor(r, rel(f.coluna)), None)) == f.excluir {
                continue 'linhas;
            }
        }
        medidas.clear();
        for v in &config.valores {
            medidas.push(Medida::de(&valor(r, rel(v.coluna))));
        }
        // As colunas: cada prefixo da chave também acumula (para ordenar por
        // valor e para os irmãos do "mostrar valores como").
        let mut chave_coluna = String::new();
        let mut no_col = &mut raiz_col;
        for (d, &c) in config.colunas.iter().enumerate() {
            let (chave, ordem) = chave_do_valor(valor(r, rel(c)), d, &extras.colunas, config.agrupar_data);
            let chave = extras.juntar(c, chave);
            if d > 0 {
                chave_coluna.push(SEPARADOR);
            }
            chave_coluna.push_str(&chave);
            no_col = no_col.filho(chave, ordem);
            empurrar(somas_de(&mut por_coluna, &chave_coluna, config), &medidas);
        }
        let mut caminho = String::new();
        let mut no = &mut raiz;
        for (d, &c) in config.linhas.iter().enumerate() {
            let (chave, ordem) = chave_do_valor(valor(r, rel(c)), d, &extras.linhas, config.agrupar_data);
            let chave = extras.juntar(c, chave);
            if d > 0 {
                caminho.push(SEPARADOR);
            }
            caminho.push_str(&chave);
            no = no.filho(chave, ordem);
            dupla.clear();
            dupla.push_str(&caminho);
            dupla.push(JUNTA);
            dupla.push_str(&chave_coluna);
            empurrar(somas_de(&mut por_caminho, &dupla, config), &medidas);
            empurrar(somas_de(&mut por_caminho_total, &caminho, config), &medidas);
        }
        empurrar(&mut tudo, &medidas);
    }

    let vazio: Numeros = acumuladores(config);
    let calc_linhas = calculados_do_eixo(&raiz, &config.linhas, extras);
    let calc_colunas = calculados_do_eixo(&raiz_col, &config.colunas, extras);
    // O valor cru de uma medida no cruzamento; "" = todos.
    let cru_base = |caminho: &str, ck: &str, i: usize| -> f64 {
        let acc = match (caminho.is_empty(), ck.is_empty()) {
            (true, true) => &tudo,
            (true, false) => por_coluna.get(ck).unwrap_or(&vazio),
            (false, true) => por_caminho_total.get(caminho).unwrap_or(&vazio),
            (false, false) => por_caminho.get(&format!("{caminho}{JUNTA}{ck}")).unwrap_or(&vazio),
        };
        acc[i].resultado()
    };
    // shortcut: totais e subtotais não somam os itens calculados (o Excel soma); mudar se pedirem.
    let cru = |caminho: &str, ck: &str, i: usize| -> f64 {
        com_calculados(caminho, &calc_linhas, &|c| com_calculados(ck, &calc_colunas, &|k| cru_base(c, k, i)))
    };
    ordenar(&mut raiz, 0, "", &extras.linhas, &|c, m| cru(c, "", m));
    ordenar(&mut raiz_col, 0, "", &extras.colunas, &|c, m| cru("", c, m));
    inserir_calculados(&mut raiz, 0, &calc_linhas);
    inserir_calculados(&mut raiz_col, 0, &calc_colunas);

    let com_colunas = !config.colunas.is_empty();
    let colunas: Vec<String> = if com_colunas {
        let mut v = Vec::new();
        folhas(&raiz_col, "", &mut v);
        v
    } else {
        vec![String::new()]
    };
    let irmaos = |eixo: Eixo, prefixo: &[String]| -> Vec<String> {
        let raiz = if eixo == Eixo::Linhas { &raiz } else { &raiz_col };
        raiz.descer(prefixo).map(|n| n.ordenados.clone()).unwrap_or_default()
    };
    let grade = Grade { cru: &cru, irmaos: &irmaos };
    let celula = |caminho: &str, ck: &str, i: usize| -> Value {
        match extras.mostrar_como(i) {
            MostrarComo::Normal => Value::Number(cru(caminho, ck, i)),
            modo => mostrar_como(modo, i, caminho, ck, &grade),
        }
    };
    let total_linha_direita = com_colunas && extras.totais_gerais_linhas;
    // As contas de uma linha: cada coluna × medida e, com campo em Colunas,
    // o total geral da linha.
    let contas = |caminho: &str| -> Vec<Value> {
        let mut v = Vec::new();
        for ck in &colunas {
            for i in 0..n_medidas {
                v.push(celula(caminho, ck, i));
            }
        }
        if total_linha_direita {
            for i in 0..n_medidas {
                v.push(celula(caminho, "", i));
            }
        }
        v
    };
    let n_valores = colunas.len() * n_medidas + if total_linha_direita { n_medidas } else { 0 };
    let brancos = || vec![Value::Text(String::new()); n_valores];

    // O cabeçalho: os rótulos e, para cada coluna × medida, o nome.
    let largura_rotulo = if opcoes.layout == Layout::Compacto { 1 } else { n.max(1) };
    let mut cab: Vec<Value> = if opcoes.layout == Layout::Compacto || n == 0 {
        vec![Value::Text("Rótulos de Linha".into())]
    } else {
        config.linhas.iter().map(|&c| Value::Text(cabecalho(c))).collect()
    };
    for ck in &colunas {
        for m in &config.valores {
            let nome = cabecalho(m.coluna);
            cab.push(Value::Text(if ck.is_empty() { nome } else { format!("{} ({nome})", ck.replace(&sep, " | ")) }));
        }
    }
    if total_linha_direita {
        for m in &config.valores {
            cab.push(Value::Text(format!("Total Geral ({})", cabecalho(m.coluna))));
        }
    }

    let mut saida = Saida { linhas: vec![cab], info: vec![InfoLinha::nova(TipoLinha::Cabecalho, 0)], caminhos: vec![None] };
    let ctx = Contexto { opcoes, n, largura_rotulo, do_caminho: &contas, brancos: &brancos };
    let mut anterior: Vec<String> = Vec::new();
    for nome in &raiz.ordenados {
        emitir(&ctx, &mut saida, nome, &raiz.filhos[nome], nome.clone(), 0, &mut anterior);
    }

    // O total geral (sem campo em Linhas, é a única linha de dados).
    if extras.totais_gerais_colunas || n == 0 {
        let mut ultima = vec![Value::Text(String::new()); largura_rotulo];
        ultima[0] = Value::Text("Total Geral".into());
        ultima.extend(contas(""));
        saida.linhas.push(ultima);
        saida.info.push(InfoLinha::nova(TipoLinha::Total, 0));
        saida.caminhos.push(Some(String::new()));
    }

    let mut chaves_colunas: Vec<Option<String>> = vec![None; largura_rotulo];
    for ck in &colunas {
        chaves_colunas.extend(std::iter::repeat(Some(ck.clone())).take(n_medidas));
    }
    if total_linha_direita {
        chaves_colunas.extend(std::iter::repeat(Some(String::new())).take(n_medidas));
    }
    Resultado {
        matriz: Matrix::new(saida.linhas),
        linhas: saida.info,
        colunas_de_rotulo: largura_rotulo,
        caminhos: saida.caminhos,
        chaves_colunas,
    }
}

struct Saida {
    linhas: Vec<Vec<Value>>,
    info: Vec<InfoLinha>,
    caminhos: Vec<Option<String>>,
}

struct Contexto<'a> {
    opcoes: &'a Opcoes,
    n: usize,
    largura_rotulo: usize,
    do_caminho: &'a dyn Fn(&str) -> Vec<Value>,
    brancos: &'a dyn Fn() -> Vec<Value>,
}

/// Emite um grupo (e o que houver embaixo dele) em profundidade `d`.
/// `anterior` é o caminho da última linha de rótulo escrita — é o que o
/// tabular sem repetição usa para deixar em branco o que já apareceu.
fn emitir(ctx: &Contexto, saida: &mut Saida, nome: &str, no: &No, caminho: String, d: usize, anterior: &mut Vec<String>) {
    let folha = d + 1 == ctx.n;
    let recolhido = !folha && ctx.opcoes.recolhidos.contains(&caminho);
    let partes: Vec<String> = caminho.split(SEPARADOR).map(str::to_string).collect();
    let marca = |coluna: usize| Marca { coluna, caminho: caminho.clone(), recolhido };

    match ctx.opcoes.layout {
        Layout::Compacto => {
            let mut linha = vec![Value::Text(nome.to_string())];
            let mut info = InfoLinha::nova(if folha { TipoLinha::Item } else { TipoLinha::Grupo }, d);
            let embaixo = ctx.opcoes.subtotais && ctx.opcoes.subtotal_embaixo && !folha && !recolhido;
            if folha || recolhido || (ctx.opcoes.subtotais && !embaixo) {
                linha.extend((ctx.do_caminho)(&caminho));
            } else {
                linha.extend((ctx.brancos)());
            }
            if !folha {
                info.marcas.push(marca(0));
            }
            saida.linhas.push(linha);
            saida.info.push(info);
            saida.caminhos.push(Some(caminho.clone()));
            if !recolhido {
                for n in &no.ordenados {
                    emitir(ctx, saida, n, &no.filhos[n], format!("{caminho}{SEPARADOR}{n}"), d + 1, anterior);
                }
            }
            if embaixo {
                let mut linha = vec![Value::Text(format!("{nome} Total"))];
                linha.extend((ctx.do_caminho)(&caminho));
                saida.linhas.push(linha);
                saida.info.push(InfoLinha::nova(TipoLinha::Subtotal, d));
                saida.caminhos.push(Some(caminho.clone()));
            }
        }
        Layout::Tabular => {
            if folha || recolhido {
                // Uma linha de rótulos: os níveis até `d`, em branco o que
                // repete a linha de cima quando não é para repetir.
                let comum = partes.iter().zip(anterior.iter()).take_while(|(a, b)| a == b).count();
                let mut linha = Vec::with_capacity(ctx.largura_rotulo);
                let mut info = InfoLinha::nova(if folha { TipoLinha::Item } else { TipoLinha::Grupo }, d);
                for (i, p) in partes.iter().enumerate() {
                    let mostrar = ctx.opcoes.repetir_rotulos || i >= comum;
                    linha.push(Value::Text(if mostrar { p.clone() } else { String::new() }));
                    // O +/− de cada grupo fica na primeira linha dele.
                    if i < ctx.n - 1 && i >= comum {
                        let c = partes[..=i].join(&SEPARADOR.to_string());
                        let rec = i == d && recolhido;
                        info.marcas.push(Marca { coluna: i, caminho: c, recolhido: rec });
                    }
                }
                linha.resize(ctx.largura_rotulo, Value::Text(String::new()));
                linha.extend((ctx.do_caminho)(&caminho));
                *anterior = partes;
                saida.linhas.push(linha);
                saida.info.push(info);
                saida.caminhos.push(Some(caminho.clone()));
            } else {
                for n in &no.ordenados {
                    emitir(ctx, saida, n, &no.filhos[n], format!("{caminho}{SEPARADOR}{n}"), d + 1, anterior);
                }
                if ctx.opcoes.subtotais {
                    let mut linha = vec![Value::Text(String::new()); ctx.largura_rotulo];
                    linha[d] = Value::Text(format!("{nome} Total"));
                    linha.extend((ctx.do_caminho)(&caminho));
                    saida.linhas.push(linha);
                    saida.info.push(InfoLinha::nova(TipoLinha::Subtotal, d));
                    saida.caminhos.push(Some(caminho.clone()));
                    // Depois do subtotal o grupo seguinte mostra os rótulos.
                    anterior.clear();
                }
            }
        }
    }
}

/// Recolher/expandir por nível (DIN-9), um passo por clique, como os botões
/// de tópicos: recolher fecha todos os grupos do nível mais fundo ainda
/// aberto; expandir abre o nível mais raso ainda fechado.
pub fn passo_de_nivel(grupos: &[String], recolhidos: &mut Vec<String>, recolher: bool) {
    let nivel = |g: &String| g.matches(SEPARADOR).count();
    if recolher {
        let Some(d) = grupos.iter().filter(|g| !recolhidos.contains(g)).map(nivel).max() else { return };
        recolhidos.extend(grupos.iter().filter(|g| nivel(g) == d && !recolhidos.contains(g)).cloned().collect::<Vec<_>>());
    } else if let Some(d) = recolhidos.iter().map(nivel).min() {
        recolhidos.retain(|g| nivel(g) != d);
    }
}

/// O mapa das dinâmicas antigas: cabeçalho, itens e o total geral.
fn mapa_antigo(matriz: &Matrix, n: usize) -> Vec<InfoLinha> {
    let altura = matriz.rows().len();
    (0..altura)
        .map(|i| {
            let tipo = if i == 0 {
                TipoLinha::Cabecalho
            } else if i + 1 == altura {
                TipoLinha::Total
            } else {
                TipoLinha::Item
            };
            InfoLinha::nova(tipo, n.saturating_sub(1))
        })
        .collect()
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::dinamica::pivot::{montar_dinamica, AgregacaoOp, MedidaDinamica};
    use crate::celulas::refs::Rect;

    /// O exemplo do usuário: `a b 3`, `a c 5`, mais um `d e 1`.
    fn dados() -> Vec<Vec<Value>> {
        let t = |s: &str| Value::Text(s.into());
        vec![
            vec![t("Grupo"), t("Item"), t("Valor"), t("Status")],
            vec![t("a"), t("b"), Value::Number(3.0), t("ok")],
            vec![t("a"), t("c"), Value::Number(5.0), t("ok")],
            vec![t("d"), t("e"), Value::Number(1.0), t("novo")],
        ]
    }

    fn config() -> DinamicaConfig {
        DinamicaConfig {
            origem: Rect::new(0, 0, 3, 3),
            linhas: vec![0, 1],
            colunas: vec![],
            valores: vec![MedidaDinamica { coluna: 2, op: AgregacaoOp::Soma }],
            agrupar_data: None,
        }
    }

    fn rodar(opcoes: &Opcoes) -> Resultado {
        let d = dados();
        montar(d.len(), |r, c| d[r].get(c).cloned().unwrap_or(Value::Empty), &config(), opcoes)
    }

    fn texto(r: &Resultado) -> Vec<Vec<String>> {
        r.matriz.rows().iter().map(|l| l.iter().map(to_text).collect()).collect()
    }

    fn compacto() -> Opcoes {
        Opcoes { layout: Layout::Compacto, subtotais: true, ..Opcoes::antigas() }
    }

    #[test]
    fn nivel_recolhe_do_mais_fundo_e_expande_do_mais_raso() {
        let s = SEPARADOR;
        let grupos: Vec<String> = vec!["a".into(), format!("a{s}x"), "b".into(), format!("b{s}y")];
        let mut r = Vec::new();
        passo_de_nivel(&grupos, &mut r, true);
        assert_eq!(r, vec![format!("a{s}x"), format!("b{s}y")]);
        passo_de_nivel(&grupos, &mut r, true);
        assert_eq!(r.len(), 4);
        passo_de_nivel(&grupos, &mut r, true);
        assert_eq!(r.len(), 4, "nada mais a recolher");
        passo_de_nivel(&grupos, &mut r, false);
        assert_eq!(r, vec![format!("a{s}x"), format!("b{s}y")]);
    }

    #[test]
    fn compacto_com_subtotal_embaixo_do_grupo() {
        let r = rodar(&Opcoes { subtotal_embaixo: true, ..compacto() });
        assert_eq!(
            texto(&r),
            vec![
                vec!["Rótulos de Linha", "Valor"],
                vec!["a", ""],
                vec!["b", "3"],
                vec!["c", "5"],
                vec!["a Total", "8"],
                vec!["d", ""],
                vec!["e", "1"],
                vec!["d Total", "1"],
                vec!["Total Geral", "9"],
            ]
        );
        assert_eq!(r.linhas[4].tipo, TipoLinha::Subtotal);
        // Recolhido, o grupo volta a ter a conta na própria linha.
        let mut o = Opcoes { subtotal_embaixo: true, ..compacto() };
        o.recolhidos.insert("a".into());
        assert_eq!(texto(&rodar(&o))[1], vec!["a", "8"]);
    }

    #[test]
    fn compacto_grupo_com_subtotal_e_itens_embaixo() {
        let r = rodar(&compacto());
        assert_eq!(
            texto(&r),
            vec![
                vec!["Rótulos de Linha", "Valor"],
                vec!["a", "8"],
                vec!["b", "3"],
                vec!["c", "5"],
                vec!["d", "1"],
                vec!["e", "1"],
                vec!["Total Geral", "9"],
            ]
        );
        let tipos: Vec<_> = r.linhas.iter().map(|i| (i.tipo, i.nivel)).collect();
        assert_eq!(
            tipos,
            vec![
                (TipoLinha::Cabecalho, 0),
                (TipoLinha::Grupo, 0),
                (TipoLinha::Item, 1),
                (TipoLinha::Item, 1),
                (TipoLinha::Grupo, 0),
                (TipoLinha::Item, 1),
                (TipoLinha::Total, 0),
            ]
        );
        assert_eq!(r.linhas[1].marcas, vec![Marca { coluna: 0, caminho: "a".into(), recolhido: false }]);
        assert_eq!(r.colunas_de_rotulo, 1);
    }

    #[test]
    fn recolher_o_grupo_mostra_so_o_total_dele() {
        let mut o = compacto();
        o.recolhidos.insert("a".into());
        let r = rodar(&o);
        assert_eq!(
            texto(&r),
            vec![
                vec!["Rótulos de Linha", "Valor"],
                vec!["a", "8"],
                vec!["d", "1"],
                vec!["e", "1"],
                vec!["Total Geral", "9"],
            ]
        );
        assert!(r.linhas[1].marcas[0].recolhido);
        assert!(!r.linhas[2].marcas[0].recolhido);
    }

    #[test]
    fn tabular_repetindo_ou_nao_os_rotulos() {
        let mut o = Opcoes { subtotais: true, ..Opcoes::antigas() };
        let r = rodar(&o);
        assert_eq!(
            texto(&r),
            vec![
                vec!["Grupo", "Item", "Valor"],
                vec!["a", "b", "3"],
                vec!["a", "c", "5"],
                vec!["a Total", "", "8"],
                vec!["d", "e", "1"],
                vec!["d Total", "", "1"],
                vec!["Total Geral", "", "9"],
            ]
        );
        o.repetir_rotulos = false;
        let r = rodar(&o);
        assert_eq!(texto(&r)[2], vec!["", "c", "5"], "o \"a\" repetido fica em branco");
        // O +/− do grupo "a" fica na primeira linha dele, na coluna do campo.
        assert_eq!(r.linhas[1].marcas, vec![Marca { coluna: 0, caminho: "a".into(), recolhido: false }]);
        assert!(r.linhas[2].marcas.is_empty());
        // Recolhido, o grupo é uma linha com o total dele.
        o.recolhidos.insert("a".into());
        let r = rodar(&o);
        assert_eq!(texto(&r)[1], vec!["a", "", "8"]);
        assert!(r.linhas[1].marcas[0].recolhido);
    }

    #[test]
    fn o_filtro_tira_as_linhas_antes_de_agrupar() {
        let mut o = compacto();
        o.filtros.push(Filtro { coluna: 3, valores: HashSet::from(["ok".to_string()]), excluir: false });
        let r = rodar(&o);
        assert_eq!(texto(&r).last().unwrap(), &vec!["Total Geral".to_string(), "8".into()]);
        assert!(texto(&r).iter().all(|l| l[0] != "d"));
        // O mesmo, dito ao contrário: sai o "novo".
        o.filtros[0] = Filtro { coluna: 3, valores: HashSet::from(["novo".to_string()]), excluir: true };
        assert_eq!(texto(&rodar(&o)), texto(&r));
    }

    #[test]
    fn filtro_sem_dimensao_de_linha_deve_ser_respeitado() {
        let dados = dados();
        let mut config = config();
        config.linhas.clear();
        let mut ops = Opcoes::antigas();
        ops.filtros.push(Filtro {
            coluna: 0, valores: ["a".to_string()].into_iter().collect(), excluir: true,
        });
        let resultado = montar(dados.len(), |r,c| dados[r][c].clone(), &config, &ops);
        assert_eq!(resultado.matriz.height(), 2);
        let valor = crate::celulas::values::to_number(resultado.matriz.at(1,1));
        assert_eq!(valor, Ok(1.));
        assert_eq!(resultado.colunas_de_rotulo, 1);
    }

    #[test]
    fn a_configuracao_antiga_sai_igual_a_de_antes() {
        let d = dados();
        let antes = montar_dinamica(&d, &config());
        let r = rodar(&Opcoes::antigas());
        assert_eq!(r.matriz.rows(), antes.rows());
        assert_eq!(r.linhas.first().unwrap().tipo, TipoLinha::Cabecalho);
        assert_eq!(r.linhas.last().unwrap().tipo, TipoLinha::Total);
    }

    #[test]
    fn colunas_cruzadas_no_compacto() {
        let mut c = config();
        c.linhas = vec![0];
        c.colunas = vec![3];
        let d = dados();
        let r = montar(d.len(), |r, k| d[r].get(k).cloned().unwrap_or(Value::Empty), &c, &compacto());
        assert_eq!(texto(&r)[0], vec!["Rótulos de Linha", "novo (Valor)", "ok (Valor)", "Total Geral (Valor)"]);
        assert_eq!(texto(&r)[1], vec!["a", "0", "8", "8"]);
    }

    #[test]
    fn coluna_de_percentual_pede_formato_de_percentual() {
        let d = dados();
        let extras = Extras { valores: vec![MostrarComo::PercTotalGeral], ..Extras::default() };
        let r = montar_com_extras(d.len(), |r, c| d[r].get(c).cloned().unwrap_or(Value::Empty),
            &config(), &compacto(), &extras);
        assert_eq!(r.colunas_de_rotulo, 1);
        assert_eq!(formato_da_coluna(&r, &extras, 1, 0), None);
        assert_eq!(formato_da_coluna(&r, &extras, 1, 1), Some("0,00%"));
        // A linha "a" soma 8 de 9.
        assert_eq!(r.matriz.rows()[1][1], Value::Number(8.0 / 9.0));
        assert_eq!(formato_da_coluna(&r, &Extras::default(), 1, 1), None);
    }

    #[test]
    fn rotulo_do_item_segue_o_agrupamento_do_campo() {
        use crate::dinamica::exibicao::{Agrupamento, Faixas};
        let extra = CampoExtra {
            agrupamento: Some(Agrupamento::Faixas(Faixas { inicio: 0.0, fim: 100.0, intervalo: 10.0 })),
            ..CampoExtra::default()
        };
        let (sem, _) = rotulo_do_item(&Value::Text("Norte".into()), &CampoExtra::default(), None);
        assert_eq!(sem, "Norte");
        let (com, ordem) = rotulo_do_item(&Value::Number(15.0), &extra, None);
        assert_eq!((com, ordem.is_some()), (crate::dinamica::exibicao::chave_agrupada(&Value::Number(15.0),
            extra.agrupamento.as_ref().unwrap()).0, true));
    }

    #[test]
    fn detalhar_acha_as_linhas_do_grupo_do_item_e_do_total() {
        let d = dados();
        let valor = |r: usize, c: usize| d[r].get(c).cloned().unwrap_or(Value::Empty);
        let (r, grupo) = linhas_da_celula(d.len(), valor, &config(), &compacto(), &Extras::default(), 1, 1)
            .expect("célula de valor");
        assert_eq!(texto(&r)[1][0], "a");
        assert_eq!(grupo, vec![1, 2], "o grupo a junta b e c");
        let item = r.caminhos.iter().position(|c| c.as_deref() == Some(&*format!("a{}c", SEPARADOR))).unwrap();
        let (_, so_c) = linhas_da_celula(d.len(), valor, &config(), &compacto(), &Extras::default(), item, 1).unwrap();
        assert_eq!(so_c, vec![2]);
        let total = r.caminhos.len() - 1;
        let (_, tudo) = linhas_da_celula(d.len(), valor, &config(), &compacto(), &Extras::default(), total, 1).unwrap();
        assert_eq!(tudo, vec![1, 2, 3]);
        assert!(linhas_da_celula(d.len(), valor, &config(), &compacto(), &Extras::default(), 1, 0).is_none(),
            "rótulo não detalha");
        assert!(linhas_da_celula(d.len(), valor, &config(), &compacto(), &Extras::default(), 0, 1).is_none(),
            "cabeçalho não detalha");
    }

    /// DIN-6: "b" e "c" agrupados à mão viram uma linha só, com a soma dos dois, e o
    /// Detalhes do grupo acha as linhas de ambos.
    #[test]
    fn itens_agrupados_a_mao_somam_juntos_e_detalham() {
        use crate::dinamica::exibicao::GrupoDeItens;
        let d = dados();
        let valor = |r: usize, c: usize| d[r].get(c).cloned().unwrap_or(Value::Empty);
        let config = DinamicaConfig { linhas: vec![1], ..config() };
        let extras = Extras { grupos_de_itens: vec![GrupoDeItens { coluna: 1, nome: "Grupo1".into(), itens: vec!["b".into(), "c".into()] }], ..Extras::default() };
        assert!(!extras.eh_padrao());
        let r = montar_com_extras(d.len(), valor, &config, &compacto(), &extras);
        let t = texto(&r);
        let linha = t.iter().position(|l| l[0] == "Grupo1").expect("a linha do grupo");
        assert_eq!(t[linha][1], "8", "3 + 5");
        assert!(!t.iter().any(|l| l[0] == "b" || l[0] == "c"), "os itens somem dentro do grupo");
        assert!(t.iter().any(|l| l[0] == "e"), "o resto fica");
        let (_, achadas) = linhas_da_celula(d.len(), valor, &config, &compacto(), &extras, linha, 1).unwrap();
        assert_eq!(achadas, vec![1, 2]);
        // Ida e volta pelo JSON da dinâmica gravada.
        let json = serde_json::to_string(&extras).unwrap();
        assert_eq!(serde_json::from_str::<Extras>(&json).unwrap(), extras);
        assert!(!serde_json::to_string(&Extras::default()).unwrap().contains("grupos_de_itens"), "sem grupo, nada a mais no arquivo");
    }

    /// DIN-6: item calculado em Linhas, em Colunas e embaixo de outro nível.
    #[test]
    fn item_calculado_faz_a_conta_sobre_os_irmaos() {
        use crate::dinamica::exibicao::ItemCalculado;
        let d = dados();
        let valor = |r: usize, c: usize| d[r].get(c).cloned().unwrap_or(Value::Empty);
        let calc = |coluna: usize, nome: &str, formula: &str| Extras {
            itens_calculados: vec![ItemCalculado { coluna, nome: nome.into(), formula: formula.into() }],
            ..Extras::default()
        };
        // Em Linhas, com Status em Colunas: c - b em cada coluna; o total geral fica sem ele.
        let config_lc = DinamicaConfig { linhas: vec![1], colunas: vec![3], ..config() };
        let t = texto(&montar_com_extras(d.len(), valor, &config_lc, &compacto(), &calc(1, "c menos b", "c - b")));
        let i = t.iter().position(|l| l[0] == "c menos b").expect("a linha do item");
        assert_eq!(t[i - 1][0], "e", "no fim dos itens");
        assert_eq!(&t[i][1..], ["0", "2", "2"], "novo, ok, total da linha");
        assert_eq!(t.last().unwrap()[3], "9", "total geral sem o item calculado");
        // Em Colunas.
        let t = texto(&montar_com_extras(d.len(), valor, &config_lc, &compacto(), &calc(3, "Dif", "[ok] - novo")));
        let b = t.iter().find(|l| l[0] == "b").unwrap();
        assert_eq!(&b[1..4], ["0", "3", "3"], "novo, ok, ok - novo");
        // No segundo nível: só embaixo do pai que tem os itens usados.
        let t = texto(&montar_com_extras(d.len(), valor, &config(), &compacto(), &calc(1, "Dobro de b", "b * 2")));
        assert_eq!(t.iter().filter(|l| l[0] == "Dobro de b").map(|l| l[1].as_str()).collect::<Vec<_>>(), ["6"]);
        assert!(!calc(1, "x", "b").eh_padrao());
    }

    #[test]
    fn a_dinamica_simples_tambem_anota_os_caminhos() {
        let d = dados();
        let valor = |r: usize, c: usize| d[r].get(c).cloned().unwrap_or(Value::Empty);
        for colunas in [vec![], vec![3]] {
            let config = DinamicaConfig { colunas, ..config() };
            let r = montar_com_extras(d.len(), valor, &config, &Opcoes::antigas(), &Extras::default());
            assert_eq!(r.caminhos.len(), r.matriz.height());
            assert_eq!(r.chaves_colunas.len(), r.matriz.rows()[0].len());
            for (i, linha) in r.matriz.rows().iter().enumerate().skip(1) {
                for (j, celula) in linha.iter().enumerate().skip(r.colunas_de_rotulo) {
                    let (Some(caminho), Some(ck)) = (&r.caminhos[i], &r.chaves_colunas[j]) else {
                        panic!("célula de valor sem caminho em ({i}, {j})");
                    };
                    let achadas = linhas_do_caminho(d.len(), valor, &config, &Opcoes::antigas(),
                        &Extras::default(), caminho, ck);
                    let soma: f64 = achadas.iter().map(|&r| crate::celulas::values::to_number(&d[r][2]).unwrap()).sum();
                    assert_eq!(Value::Number(soma), *celula, "({i}, {j}) com {:?}", config.colunas);
                }
            }
        }
    }
}

