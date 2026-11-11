//! Por que a célula deu erro: o trecho da fórmula que estourou, o caminho de
//! célula em célula até a origem, e o que fazer.
//!
//! Tradução do `explain.ts`. Lá o avaliador do TypeScript rodava contra a
//! planilha do Rust, lendo célula a célula pela ponte; aqui tudo acontece na
//! pasta viva.

use crate::celulas::number::to_js_string;
use crate::celulas::refs::{cell_name, format_ref, Rect};
use crate::celulas::values::{CellError, ErrorCode, Value};
use crate::formula::ast::{Node, UnaryOp};
use crate::formula::formula_text::quote_sheet;
use crate::formula::parser::parse_formula;
use crate::planilha::workbook::Workbook;

/// Um elo do caminho: a célula, a fórmula dela e o trecho que estourou.
#[derive(Clone, Debug, PartialEq)]
pub struct Passo {
    /// "A1", ou "Dados!A1" quando o erro atravessa abas.
    pub cell: String,
    pub formula: String,
    /// Pedaço da fórmula que produziu o erro.
    pub culprit: String,
    /// "2º argumento de SOMA", quando dá para apontar.
    pub onde: Option<String>,
    /// Célula culpada dentro deste passo ("A2"), quando existe uma.
    pub blame_cell: Option<String>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct Explicacao {
    pub code: &'static str,
    pub meaning: String,
    pub summary: String,
    /// Do topo até a origem; o último passo é onde o erro nasceu.
    pub steps: Vec<Passo>,
    pub hint: String,
}

/// O que cada código quer dizer, e a dica de sempre.
/// O que o erro quer dizer e o que fazer, na língua do app.
fn significado(code: ErrorCode) -> (String, String) {
    let (o_que, como) = significado_pt(code);
    (crate::tr!(o_que), crate::tr!(como))
}

fn significado_pt(code: ErrorCode) -> (&'static str, &'static str) {
    match code {
        ErrorCode::Div0 => (
            "Uma divisão tem zero (ou uma célula vazia) como divisor.",
            "Proteja a conta com SEERRO, ou garanta que o divisor nunca seja zero.",
        ),
        ErrorCode::Value => (
            "Um argumento é do tipo errado — texto onde se espera número, em geral.",
            "Confira o valor apontado: se for um número guardado como texto, use VALOR() ou corrija a célula.",
        ),
        ErrorCode::Ref => (
            "A fórmula aponta para um lugar que não existe mais.",
            "A linha, a coluna ou a aba referenciada foi excluída: reescreva a referência.",
        ),
        ErrorCode::Error => (
            "A fórmula não chegou a ser lida: há algo escrito que a planilha não entende.",
            "Corrija o trecho apontado — em geral é parêntese ou aspas sem fechar.",
        ),
        ErrorCode::Name => (
            "Há um nome que a planilha não conhece: função escrita errado ou intervalo nomeado inexistente.",
            "Verifique a grafia do nome; a lista completa está no catálogo de funções (fx).",
        ),
        ErrorCode::Num => (
            "A conta não tem resultado possível para os números informados.",
            "Confira os limites do argumento (raiz de negativo, k fora do intervalo, data final antes da inicial).",
        ),
        ErrorCode::Na => (
            "O valor procurado não foi encontrado.",
            "Confira se o procurado existe mesmo na primeira coluna do intervalo, sem espaços sobrando.",
        ),
        ErrorCode::Spill => (
            "O resultado é uma matriz e não há espaço livre para derramar.",
            "Limpe as células à direita e abaixo da fórmula.",
        ),
        ErrorCode::Circular => (
            "A fórmula depende dela mesma, direta ou indiretamente.",
            "Refaça a conta para que o ciclo se quebre: nenhuma célula pode se referenciar de volta.",
        ),
        ErrorCode::Null => (
            "Dois intervalos que deviam se cruzar não têm nenhuma célula em comum.",
            "Confira os intervalos: entre eles vai ; ou :, não um espaço.",
        ),
        ErrorCode::Calculating => (
            "Não é erro: a resposta ainda está a caminho.",
            "A célula se atualiza sozinha quando o resultado chegar.",
        ),
    }
}

/// Até onde seguir a cadeia de precedentes (e proteção contra ciclo).
const MAX_PASSOS: usize = 12;

/// Quantos caracteres mostrar de cada lado do ponto do erro de escrita.
const EM_VOLTA: usize = 24;

/// Onde a explicação está: a aba (índice) e a célula.
#[derive(Clone, Copy)]
struct Local {
    aba: usize,
    row: u32,
    col: u32,
}

/// Uma referência encontrada na fórmula, ainda com o nome da aba.
struct Alvo {
    row: u32,
    col: u32,
    sheet: Option<String>,
}

#[derive(Default)]
struct Culpa {
    next: Option<Alvo>,
    onde: Option<String>,
    cell: Option<String>,
}

/// O avaliador de uma célula: cada trecho é calculado no lugar dela.
struct Contexto<'a> {
    pasta: &'a Workbook,
    em: Local,
}

impl Contexto<'_> {
    fn eval(&self, node: &Node) -> Value {
        self.pasta.avaliar_em(self.em.aba, self.em.row, self.em.col, node)
    }

    /// A aba de um intervalo ou referência: a própria, ou a citada pelo nome.
    fn aba_de(&self, sheet: &Option<String>) -> Option<usize> {
        match sheet {
            None => Some(self.em.aba),
            Some(nome) => self.pasta.indice_por_nome(nome),
        }
    }

    /// As células de um intervalo, cortado à área usada da aba (o intervalo
    /// de coluna inteira não varre um milhão de linhas vazias).
    fn celulas(&self, r0: u32, c0: u32, r1: u32, c1: u32, sheet: &Option<String>, mut visitar: impl FnMut(u32, u32, Value) -> bool) {
        let Some(aba) = self.aba_de(sheet) else { return };
        let Some(viva) = self.pasta.aba(aba) else { return };
        let usado = viva.sheet.used_rect();
        let (r1, c1) = (r1.min(usado.r1), c1.min(usado.c1));
        for r in r0..=r1 {
            for c in c0..=c1 {
                if !visitar(r, c, self.pasta.valor(aba, r, c)) {
                    return;
                }
            }
        }
    }
}

fn erro_de(v: &Value) -> Option<&CellError> {
    match v {
        Value::Error(e) => Some(e),
        _ => None,
    }
}

/// Explica por que a célula está com erro. `None` quando ela não tem erro.
pub fn explicar_erro(pasta: &Workbook, aba: usize, row: u32, col: u32) -> Option<Explicacao> {
    let valor = pasta.valor(aba, row, col);
    let mut ultimo = erro_de(&valor)?.clone();

    let mut passos: Vec<Passo> = Vec::new();
    let mut vistos: Vec<String> = Vec::new();
    let mut atual = Some(Local { aba, row, col });

    while let Some(local) = atual {
        if passos.len() >= MAX_PASSOS {
            break;
        }
        let nome = nome_da_celula(pasta, aba, local);
        if vistos.contains(&nome) {
            break;
        }
        vistos.push(nome.clone());

        let sheet = &pasta.aba(local.aba)?.sheet;
        let raw = sheet.get_raw(local.row, local.col);
        let tem_celula = sheet.has_content(local.row, local.col);
        let aqui = pasta.valor(local.aba, local.row, local.col);
        if let Some(e) = erro_de(&aqui) {
            ultimo = e.clone();
        }

        if let Some(escrita) = problema_de_escrita(&raw) {
            passos.push(Passo { cell: nome, formula: raw, culprit: escrita.trecho.clone(), onde: Some(escrita.onde.clone()), blame_cell: None });
            let (meaning, _) = significado(ErrorCode::Error);
            return Some(Explicacao {
                code: ErrorCode::Error.as_str(),
                meaning,
                summary: format!("{}, {}: {}", escrita.mensagem, escrita.onde, escrita.trecho),
                steps: passos,
                hint: escrita.dica,
            });
        }

        let arvore = if tem_celula && raw.starts_with('=') && raw.chars().count() >= 2 {
            parse_formula(&raw[1..]).ok()
        } else {
            None
        };
        let Some(arvore) = arvore else {
            // Fim da linha: uma célula sem fórmula. O que ela guarda entra
            // como detalhe do passo anterior, onde a conta de fato quebrou.
            match passos.last_mut() {
                Some(anterior) => anterior.onde = Some(crate::trf!("{nome} contém {}", descrever(&aqui, &raw))),
                None => {
                    let culprit = if tem_celula { raw.clone() } else { crate::operacoes::filtro::texto_js(&aqui) };
                    passos.push(Passo { cell: nome, formula: raw, culprit, onde: None, blame_cell: None });
                }
            }
            break;
        };

        let ctx = Contexto { pasta, em: local };
        let no = origem(&arvore, &ctx).unwrap_or(&arvore);
        let erro_aqui = erro_de(&aqui).cloned();
        let culpa = culpa_em(no, &ctx, erro_aqui.as_ref());
        let mut passo = Passo { cell: nome, formula: raw, culprit: escrever(no), onde: None, blame_cell: None };
        if let Some(c) = &culpa {
            passo.onde = c.onde.clone();
            passo.blame_cell = c.cell.clone();
        }
        passos.push(passo);

        atual = culpa.and_then(|c| c.next).and_then(|alvo| {
            let aba = ctx.aba_de(&alvo.sheet)?;
            Some(Local { aba, row: alvo.row, col: alvo.col })
        });
    }

    let (meaning, _) = significado(ultimo.code);
    Some(Explicacao {
        code: ultimo.code.as_str(),
        meaning,
        summary: resumir(&ultimo, &passos),
        hint: dica(&ultimo, &passos),
        steps: passos,
    })
}

fn nome_da_celula(pasta: &Workbook, origem: usize, em: Local) -> String {
    let prefixo = if em.aba == origem {
        String::new()
    } else {
        pasta.aba(em.aba).map(|a| format!("{}!", quote_sheet(&a.sheet.name))).unwrap_or_default()
    };
    prefixo + &cell_name(em.row, em.col)
}

/// O primeiro texto entre aspas duplas (`/"([^"]+)"/`).
fn entre_aspas(texto: &str) -> Option<&str> {
    let ini = texto.find('"')? + 1;
    let fim = ini + texto[ini..].find('"')?;
    (fim > ini).then(|| &texto[ini..fim])
}

/// A dica do código, afinada pelo que se descobriu no caminho.
fn dica(erro: &CellError, passos: &[Passo]) -> String {
    let detalhe = erro.detail.as_deref().unwrap_or("");
    let fundo = passos.last();
    let culpada = fundo.and_then(|p| p.blame_cell.as_deref());
    if erro.code == ErrorCode::Value && (crate::idioma::veio_de(detalhe, "\"{texto}\" não é número") || detalhe.contains("não um número")) {
        return match culpada {
            Some(c) => crate::trf!("{c} guarda texto, e essa conta só aceita número. Confira se o intervalo é o certo — colunas de texto precisam ficar de fora."),
            None => crate::tr!("A conta pegou texto onde esperava número. Confira o intervalo: colunas de texto precisam ficar de fora."),
        };
    }
    if erro.code == ErrorCode::Value && crate::idioma::veio_de(detalhe, "As dimensões não se encaixam") {
        return crate::tr!("As matrizes não se encaixam: as colunas da primeira têm de ser tantas quanto as linhas da segunda.");
    }
    if erro.code == ErrorCode::Num && detalhe.contains("inversa") {
        return crate::tr!("A matriz não tem inversa (colunas repetidas ou dependentes). Confira se há coluna duplicada nos dados.");
    }
    if erro.code == ErrorCode::Div0 {
        if let Some(c) = culpada {
            return crate::trf!("{c} está em zero ou vazia. Proteja a conta com SEERRO, ou garanta um divisor diferente de zero.");
        }
    }
    significado(erro.code).1.to_string()
}

struct ProblemaDeEscrita {
    mensagem: String,
    onde: String,
    trecho: String,
    dica: String,
}

/// Dicas para os enroscos de escrita mais comuns.
fn dica_de_escrita(ch: Option<char>, mensagem: &str) -> String {
    if matches!(ch, Some('[') | Some(']')) {
        return crate::tr!("Colchetes em referências de tabela devem ser fechados corretamente.");
    }
    if (crate::idioma::veio_de(mensagem, "Matriz vazia") || crate::idioma::veio_de(mensagem, "Matriz com linhas de tamanhos diferentes")) {
        return crate::tr!("Dentro de { }, \";\" separa linhas e \"\\\" separa colunas, e todas as linhas precisam do mesmo tamanho.");
    }
    if crate::idioma::veio_de(mensagem, "Texto sem aspas de fechamento") {
        return crate::tr!("Falta fechar as aspas do texto.");
    }
    if crate::idioma::veio_de(mensagem, "Sobra \"{}\" no fim da fórmula") {
        return crate::tr!("Sobrou coisa depois do fim da fórmula: confira os parênteses.");
    }
    if crate::idioma::veio_de(mensagem, "Fórmula incompleta perto de \"{achei}\"") {
        return crate::tr!("A fórmula termina no meio: falta um argumento ou um fechamento.");
    }
    significado(ErrorCode::Error).1.to_string()
}

/// Releitura da fórmula só para achar onde o texto quebrou.
fn problema_de_escrita(raw: &str) -> Option<ProblemaDeEscrita> {
    let corpo = raw.strip_prefix('=')?;
    let e = parse_formula(corpo).err()?;
    let Some(at) = e.at else {
        return Some(ProblemaDeEscrita { dica: dica_de_escrita(None, &e.message), mensagem: e.message, onde: crate::tr!("na fórmula").into(), trecho: raw.into() });
    };
    // Contado em caracteres, como o `at` do analisador.
    let letras: Vec<char> = corpo.chars().collect();
    let ch = letras.get(at).copied();
    let de = at.saturating_sub(EM_VOLTA);
    let ate = (at + EM_VOLTA).min(letras.len());
    let pedaco = |a: usize, b: usize| -> String { letras.get(a..b.max(a)).map(|s| s.iter().collect()).unwrap_or_default() };
    let trecho = format!(
        "{}{}»{}«{}{}",
        if de > 0 { "…" } else { "" },
        pedaco(de, at),
        ch.map(String::from).unwrap_or_default(),
        pedaco((at + 1).min(letras.len()), ate),
        if ate < letras.len() { "…" } else { "" },
    );
    // +2: o corpo começa em zero e a pessoa conta o "=" como caractere 1.
    Some(ProblemaDeEscrita { dica: dica_de_escrita(ch, &e.message), mensagem: e.message, onde: crate::trf!("na posição {}", at + 2), trecho })
}

/// Frase do caso concreto, montada a partir do último passo.
fn resumir(erro: &CellError, passos: &[Passo]) -> String {
    let Some(fundo) = passos.last() else {
        return erro.detail.clone().unwrap_or_else(|| significado(erro.code).0.to_string());
    };
    let la = if passos.len() > 1 { crate::trf!(", lá em {}", fundo.cell) } else { String::new() };
    let trecho = crate::trf!("{} nasce em `{}`{la}", erro.code.as_str(), fundo.culprit);
    let arg = fundo.onde.as_ref().map(|o| format!(" ({o})")).unwrap_or_default();
    // Quando o "onde" já cita o valor culpado, repetir o detalhe só cansa.
    let citado = erro.detail.as_deref().and_then(entre_aspas);
    let repetido = citado.is_some_and(|c| fundo.onde.as_deref().is_some_and(|o| o.contains(c)));
    match &erro.detail {
        Some(d) if !d.is_empty() && !repetido => format!("{trecho}{arg}: {d}."),
        _ => format!("{trecho}{arg}."),
    }
}

/// O nó mais fundo que já devolve erro: o de fora costuma ser só a
/// propagação do de dentro, e é o de dentro que interessa mostrar.
fn origem<'n>(no: &'n Node, ctx: &Contexto) -> Option<&'n Node> {
    erro_de(&ctx.eval(no))?;
    for filho in filhos(no) {
        if let Some(mais_fundo) = origem(filho, ctx) {
            return Some(mais_fundo);
        }
    }
    Some(no)
}

fn filhos(no: &Node) -> Vec<&Node> {
    match no {
        Node::Call { args, .. } => args.iter().collect(),
        Node::Array { rows } => rows.iter().flatten().collect(),
        Node::Binary { left, right, .. } => vec![left, right],
        Node::Unary { operand, .. } => vec![operand],
        _ => Vec::new(),
    }
}

/// De quem é a culpa dentro do nó de origem.
fn culpa_em(no: &Node, ctx: &Contexto, erro: Option<&CellError>) -> Option<Culpa> {
    match no {
        Node::Ref { row, col, sheet } => Some(Culpa {
            next: Some(Alvo { row: *row, col: *col, sheet: sheet.clone() }),
            cell: Some(cell_name(*row, *col)),
            onde: None,
        }),
        Node::Range { .. } => {
            let ruim = primeiro_erro_nos_intervalos(std::slice::from_ref(no), ctx)?;
            let cell = cell_name(ruim.row, ruim.col);
            Some(Culpa { onde: Some(crate::trf!("a célula {cell} dentro de {}", escrever(no))), cell: Some(cell), next: Some(ruim) })
        }
        Node::Binary { op, left, right } => {
            if *op == crate::formula::ast::BinaryOp::Div {
                let divisor = ctx.eval(right);
                if matches!(divisor, Value::Empty) || matches!(divisor, Value::Number(n) if n == 0.0) {
                    let mut c = Culpa { onde: Some(crate::trf!("o divisor `{}`", escrever(right))), ..Culpa::default() };
                    if let Node::Ref { row, col, .. } = right.as_ref() {
                        c.cell = Some(cell_name(*row, *col));
                    }
                    return Some(c);
                }
            }
            let culpado = operando_culpado([left.as_ref(), right.as_ref()], ctx, erro)?;
            let mut c = Culpa { onde: Some(format!("`{}`", escrever(culpado))), ..Culpa::default() };
            if let Node::Ref { row, col, sheet } = culpado {
                c.next = Some(Alvo { row: *row, col: *col, sheet: sheet.clone() });
            }
            Some(c)
        }
        Node::Call { name, args } => {
            let mut c = Culpa::default();
            if let Some(i) = argumento_culpado(args, ctx, erro) {
                c.onde = Some(crate::trf!("{}º argumento de {name}", i + 1));
            }
            // Um intervalo não vira erro sozinho: quem estraga é uma célula dentro dele.
            if let Some(ruim) = primeiro_erro_nos_intervalos(args, ctx) {
                c.cell = Some(cell_name(ruim.row, ruim.col));
                c.next = Some(ruim);
                if c.onde.is_none() {
                    c.onde = Some(crate::trf!("intervalo lido por {name}"));
                }
            } else if let Some(citado) = erro.and_then(|e| e.detail.as_deref()).and_then(entre_aspas) {
                // O erro citou um texto: procura de qual célula ele veio.
                if let Some(fonte) = achar_nos_intervalos(args, ctx, citado) {
                    let cell = cell_name(fonte.row, fonte.col);
                    c.onde = Some(crate::trf!("{cell} tem o texto \"{citado}\""));
                    c.cell = Some(cell);
                    c.next = None;
                }
            }
            Some(c)
        }
        _ => None,
    }
}

/// O texto do valor, se for texto e o erro o citou.
fn citado_pelo_erro(v: &Value, erro: Option<&CellError>) -> bool {
    match (v, erro.and_then(|e| e.detail.as_deref())) {
        (Value::Text(t), Some(d)) => d.contains(&format!("\"{t}\"")),
        _ => false,
    }
}

/// Qual argumento estragou a chamada: o que dá erro, ou o texto que o
/// próprio erro citou ("abc" não é número).
fn argumento_culpado(args: &[Node], ctx: &Contexto, erro: Option<&CellError>) -> Option<usize> {
    args.iter().position(|a| {
        let v = ctx.eval(a);
        erro_de(&v).is_some() || citado_pelo_erro(&v, erro)
    })
}

/// O operando que estragou a conta: o que dá erro, ou o texto citado.
fn operando_culpado<'n>(lados: [&'n Node; 2], ctx: &Contexto, erro: Option<&CellError>) -> Option<&'n Node> {
    lados.into_iter().find(|lado| {
        let v = ctx.eval(lado);
        erro_de(&v).is_some() || citado_pelo_erro(&v, erro)
    })
}

/// Como citar o conteúdo de uma célula sem fórmula.
fn descrever(v: &Value, raw: &str) -> String {
    match v {
        _ if matches!(v, Value::Empty) || raw.is_empty() => crate::tr!("está vazia"),
        Value::Text(t) => crate::trf!("o texto \"{t}\""),
        Value::Number(n) => crate::trf!("o número {}", to_js_string(*n).replace('.', ",")),
        Value::Bool(b) => crate::celulas::locale::texto_logico(*b),
        Value::Error(e) => e.code.as_str().to_string(),
        Value::Empty => crate::tr!("está vazia"),
    }
}

/// A primeira célula com erro dentro dos intervalos lidos pelos argumentos.
fn primeiro_erro_nos_intervalos(args: &[Node], ctx: &Contexto) -> Option<Alvo> {
    for arg in args {
        let Node::Range { r0, c0, r1, c1, sheet } = arg else { continue };
        let mut achou = None;
        ctx.celulas(*r0, *c0, *r1, *c1, sheet, |r, c, v| {
            if erro_de(&v).is_some() {
                achou = Some(Alvo { row: r, col: c, sheet: sheet.clone() });
                return false;
            }
            true
        });
        if achou.is_some() {
            return achou;
        }
    }
    None
}

/// Onde, nos intervalos lidos (em qualquer profundidade), está o texto.
fn achar_nos_intervalos(args: &[Node], ctx: &Contexto, texto: &str) -> Option<Alvo> {
    for arg in args {
        let mut intervalos = Vec::new();
        arg.walk(&mut |n| {
            if matches!(n, Node::Range { .. }) {
                intervalos.push(n.clone());
            }
        });
        for n in intervalos {
            let Node::Range { r0, c0, r1, c1, sheet } = &n else { continue };
            let mut achou = None;
            ctx.celulas(*r0, *c0, *r1, *c1, sheet, |r, c, v| {
                if matches!(&v, Value::Text(t) if t == texto) {
                    achou = Some(Alvo { row: r, col: c, sheet: sheet.clone() });
                    return false;
                }
                true
            });
            if achou.is_some() {
                return achou;
            }
        }
    }
    None
}

/// Reescreve um pedaço da árvore como fórmula, para citar na explicação.
pub fn escrever(no: &Node) -> String {
    match no {
        Node::Num(n) => to_js_string(*n).replace('.', ","),
        Node::Str(s) => format!("\"{s}\""),
        Node::Bool(b) => crate::celulas::locale::texto_logico(*b),
        Node::Error(code) => code.clone(),
        Node::Blank => String::new(),
        Node::Name(n) => n.clone(),
        Node::Ref { row, col, sheet } => {
            sheet.as_ref().map(|s| format!("{}!", quote_sheet(s))).unwrap_or_default() + &cell_name(*row, *col)
        }
        Node::Range { r0, c0, r1, c1, sheet } => {
            let mut r = Rect::new(*r0, *c0, *r1, *c1);
            r.sheet = sheet.clone();
            format_ref(&r)
        }
        Node::Call { name, args } => format!("{name}({})", args.iter().map(escrever).collect::<Vec<_>>().join("; ")),
        Node::Array { rows } => format!(
            "{{{}}}",
            rows.iter().map(|l| l.iter().map(escrever).collect::<Vec<_>>().join("\\")).collect::<Vec<_>>().join("; ")
        ),
        Node::Binary { op, left, right } => format!("{}{}{}", escrever(left), op.as_str(), escrever(right)),
        Node::Unary { op, operand } => match op {
            UnaryOp::Percent => format!("{}%", escrever(operand)),
            outro => format!("{}{}", outro.as_str(), escrever(operand)),
        },
    }
}
