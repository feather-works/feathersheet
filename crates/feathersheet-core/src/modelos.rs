//! Modelos (templates) de planilha prontos, feitos em código Rust.
//!
//! A página inicial lista com [`lista`] e abre um com [`criar`]. Todos seguem o
//! mesmo desenho: faixa de título na cor do modelo, cartões de indicadores,
//! tabela com cabeçalho colorido e linhas listradas, linha de total e
//! formatação condicional (situação colorida, barras de dados, escala de cor).
//! Larguras e alturas em pixels, como a grade lê. Nada aqui lê arquivo externo.

use crate::celulas::estilo_parcial::{Campo, EstiloParcial};
use crate::celulas::refs::{col_name as letra, Rect};
use crate::planilha::sheet::Sheet;
use crate::planilha::workbook::Workbook;
use std::collections::BTreeMap;

/// Um item da lista de modelos: o `id` que [`criar`] aceita e o que a pessoa vê.
pub struct ModeloInfo {
    pub id: &'static str,
    pub nome: &'static str,
    pub descricao: &'static str,
}

const LISTA: &[ModeloInfo] = &[
    ModeloInfo {
        id: "em-branco",
        nome: "Pasta de trabalho em branco",
        descricao: "Uma planilha vazia, para começar do zero.",
    },
    ModeloInfo {
        id: "orcamento-domestico",
        nome: "Orçamento doméstico",
        descricao: "Receitas e despesas, previsto e realizado, saldo do mês.",
    },
    ModeloInfo {
        id: "controle-gastos",
        nome: "Controle de gastos",
        descricao: "Lançamentos do mês com resumo por categoria.",
    },
    ModeloInfo {
        id: "fluxo-caixa",
        nome: "Fluxo de caixa",
        descricao: "Entradas e saídas mês a mês, saldo acumulado no ano.",
    },
    ModeloInfo {
        id: "lista-tarefas",
        nome: "Lista de tarefas",
        descricao: "Responsável, prazo, prioridade e status coloridos.",
    },
    ModeloInfo {
        id: "inventario",
        nome: "Controle de estoque",
        descricao: "Quantidades, custos e alerta de reposição.",
    },
    ModeloInfo {
        id: "notas-escolares",
        nome: "Boletim de notas",
        descricao: "Notas com escala de cor, médias e aprovação.",
    },
    ModeloInfo {
        id: "calendario-mensal",
        nome: "Calendário do mês",
        descricao: "O mês atual, com o dia de hoje em destaque.",
    },
    ModeloInfo {
        id: "agenda-semanal",
        nome: "Agenda da semana",
        descricao: "Horários de segunda a domingo da semana atual.",
    },
    ModeloInfo {
        id: "queda-tensao",
        nome: "Queda de tensão (NBR 5410)",
        descricao: "Circuitos elétricos, queda em V e %, situação e seção mínima.",
    },
    ModeloInfo {
        id: "vendas",
        nome: "Relatório de vendas",
        descricao: "Vendas por vendedor e região, pronto para tabela dinâmica.",
    },
];

/// Os modelos disponíveis, o em branco primeiro.
pub fn lista() -> &'static [ModeloInfo] {
    LISTA
}

/// Uma pasta de trabalho nova a partir do modelo `id`, ou `None` se não existir.
pub fn criar(id: &str) -> Option<Workbook> {
    Some(match id {
        "em-branco" => Workbook::new(),
        "orcamento-domestico" => orcamento_domestico(),
        "controle-gastos" => controle_gastos(),
        "fluxo-caixa" => fluxo_caixa(),
        "lista-tarefas" => lista_tarefas(),
        "inventario" => inventario(),
        "notas-escolares" => notas_escolares(),
        "calendario-mensal" => calendario_mensal(),
        "agenda-semanal" => agenda_semanal(),
        "vendas" => vendas(),
        "queda-tensao" => queda_tensao::criar(),
        _ => return None,
    })
}

// ------------------------------------------------------------ paleta

/// As três cores de um modelo: a forte (faixa, cabeçalho, números dos
/// indicadores), a clara (cartões e total) e a da listra das linhas.
#[derive(Clone, Copy)]
struct Tema {
    forte: &'static str,
    clara: &'static str,
    listra: &'static str,
}

const VERDE: Tema = Tema { forte: "#1E7A4C", clara: "#E3F3EA", listra: "#F5FBF7" };
const LARANJA: Tema = Tema { forte: "#C2410C", clara: "#FDEBDD", listra: "#FFF8F3" };
const AZUL: Tema = Tema { forte: "#1D4ED8", clara: "#E2EAFC", listra: "#F6F9FF" };
const ROXO: Tema = Tema { forte: "#6D28D9", clara: "#EEE7FD", listra: "#FAF7FF" };
const PETROLEO: Tema = Tema { forte: "#0F766E", clara: "#DDF1EE", listra: "#F4FBFA" };
const INDIGO: Tema = Tema { forte: "#4338CA", clara: "#E7E6FA", listra: "#F7F7FE" };
const ROSA: Tema = Tema { forte: "#BE185D", clara: "#FBE4EE", listra: "#FDF3F8" };
const CIANO: Tema = Tema { forte: "#0E7490", clara: "#DDF0F6", listra: "#F4FAFC" };

const TEXTO: &str = "#1F2937";
const APAGADO: &str = "#6B7280";
const LINHA: &str = "#E5E7EB";
const BRANCO: &str = "#FFFFFF";
/// Situações: (fundo, texto).
const BOM: (&str, &str) = ("#DCFCE7", "#166534");
const ATENCAO: (&str, &str) = ("#FEF3C7", "#92400E");
const RUIM: (&str, &str) = ("#FEE2E2", "#B91C1C");
const NEUTRO: (&str, &str) = ("#F3F4F6", "#374151");

/// Altura das linhas, em pixels.
const ALTURA_TITULO: u32 = 50;
const ALTURA_SUBTITULO: u32 = 26;
const ALTURA_RESPIRO: u32 = 14;
const ALTURA_ROTULO: u32 = 24;
const ALTURA_INDICADOR: u32 = 40;
const ALTURA_CABECALHO: u32 = 32;
const ALTURA_LINHA: u32 = 27;

/// Formato de uma coluna de dados.
#[derive(Clone, Copy, PartialEq)]
enum Fmt {
    Texto,
    Centro,
    Moeda,
    Data,
    Pct,
    Inteiro,
    /// Uma casa decimal, centralizada.
    Nota,
}

// ------------------------------------------------------- pincel

/// Encadeia campos de estilo: `estilo().fundo(..).cor(..).negrito()`.
trait Pincel: Sized {
    fn campo(self, k: &str, v: Campo) -> Self;
    fn fundo(self, cor: &str) -> Self { self.campo("bg", Campo::Texto(cor.into())) }
    fn cor(self, cor: &str) -> Self { self.campo("color", Campo::Texto(cor.into())) }
    fn negrito(self) -> Self { self.campo("bold", Campo::Logico(true)) }
    fn italico(self) -> Self { self.campo("italic", Campo::Logico(true)) }
    fn tamanho(self, pt: f64) -> Self { self.campo("fontSize", Campo::Numero(pt)) }
    fn alinhar(self, a: &str) -> Self { self.campo("align", Campo::Texto(a.into())) }
    fn topo(self) -> Self { self.campo("vAlign", Campo::Texto("top".into())) }
    fn quebrar(self) -> Self { self.campo("wrap", Campo::Logico(true)) }
    fn borda(self, lados: &[&str], cor: &str) -> Self {
        let mut o = BTreeMap::new();
        for l in lados {
            o.insert((*l).to_string(), Campo::Logico(true));
        }
        o.insert("color".into(), Campo::Texto(cor.into()));
        self.campo("borders", Campo::Objeto(o))
    }
    fn formato(self, f: Fmt) -> Self {
        let num = |s: Self, nome: &str| s.campo("format", Campo::Texto(nome.into()));
        match f {
            Fmt::Texto => self,
            Fmt::Centro => self.alinhar("center"),
            Fmt::Moeda => num(self, "currency"),
            Fmt::Data => num(self, "date").alinhar("center"),
            Fmt::Pct => num(self, "percent"),
            Fmt::Inteiro => num(self, "integer").alinhar("center"),
            Fmt::Nota => num(self, "number").campo("decimals", Campo::Numero(1.0)).alinhar("center"),
        }
    }
}

impl Pincel for EstiloParcial {
    fn campo(mut self, k: &str, v: Campo) -> Self {
        self.insert(k.into(), v);
        self
    }
}

fn estilo() -> EstiloParcial {
    EstiloParcial::new()
}

// ------------------------------------------------------- referências

/// Referência A1 a partir de linha e coluna base zero: `(7, 1)` -> `"B8"`.
fn a1(r: u32, c: u32) -> String {
    format!("{}{}", letra(c), r + 1)
}

/// Intervalo A1 de uma coluna: `col(1, 7, 20)` -> `"B8:B21"`.
fn col(c: u32, r0: u32, r1: u32) -> String {
    format!("{}:{}", a1(r0, c), a1(r1, c))
}

/// Como [`col`], com `$`: `"$B$8:$B$21"`.
fn col_abs(c: u32, r0: u32, r1: u32) -> String {
    format!("${}${}:${}${}", letra(c), r0 + 1, letra(c), r1 + 1)
}

fn json_texto(s: &str) -> String {
    let mut o = String::with_capacity(s.len() + 2);
    o.push('"');
    for ch in s.chars() {
        match ch {
            '"' => o.push_str("\\\""),
            '\\' => o.push_str("\\\\"),
            _ => o.push(ch),
        }
    }
    o.push('"');
    o
}

fn linha(celulas: &[&str]) -> Vec<String> {
    celulas.iter().map(|s| s.to_string()).collect()
}

// ------------------------------------------------------- folha

/// A aba sendo montada: escreve, pinta e junta as mesclas e as regras
/// condicionais, gravadas no fim em [`Folha::concluir`].
struct Folha<'a> {
    s: &'a mut Sheet,
    t: Tema,
    mesclas: Vec<[u32; 4]>,
    regras: Vec<String>,
}

impl Folha<'_> {
    fn celula(&mut self, r: u32, c: u32, raw: &str, e: EstiloParcial) {
        if !raw.is_empty() {
            self.s.set_cell(r, c, raw);
        }
        if !e.is_empty() {
            self.s.set_style(&Rect::new(r, c, r, c), &e);
        }
    }

    fn pintar(&mut self, r0: u32, c0: u32, r1: u32, c1: u32, e: &EstiloParcial) {
        self.s.set_style(&Rect::new(r0, c0, r1, c1), e);
    }

    fn mesclar(&mut self, r0: u32, c0: u32, r1: u32, c1: u32) {
        if r1 > r0 || c1 > c0 {
            self.mesclas.push([r0, c0, r1, c1]);
        }
    }

    fn larguras(&mut self, px: &[u32]) {
        for (i, w) in px.iter().enumerate() {
            self.s.col_widths.insert(i as u32, *w);
        }
    }

    fn altura(&mut self, r: u32, px: u32) {
        self.s.row_heights.insert(r, px);
    }

    /// Faixa de título (linha 1), subtítulo (linha 2) e respiro (linha 3),
    /// de A até a coluna `ultima`. `titulo` pode ser fórmula.
    fn titulo(&mut self, ultima: u32, titulo: &str, subtitulo: &str) {
        let forte = self.t.forte;
        let clara = self.t.clara;
        self.pintar(0, 0, 0, ultima, &estilo().fundo(forte).cor(BRANCO).negrito().tamanho(20.0));
        self.celula(0, 0, titulo, estilo());
        self.mesclar(0, 0, 0, ultima);
        self.pintar(1, 0, 1, ultima, &estilo().fundo(forte).cor(clara).tamanho(11.0).italico());
        self.celula(1, 0, subtitulo, estilo());
        self.mesclar(1, 0, 1, ultima);
        self.altura(0, ALTURA_TITULO);
        self.altura(1, ALTURA_SUBTITULO);
        self.altura(2, ALTURA_RESPIRO);
    }

    /// Cartões de indicadores nas linhas `r` (rótulo) e `r + 1` (valor).
    /// Cada cartão vai da coluna `c0` à `c1`, com uma tarja à esquerda.
    fn indicadores(&mut self, r: u32, cartoes: &[(u32, u32, &str, String, Fmt)]) {
        let Tema { forte, clara, .. } = self.t;
        for (c0, c1, rotulo, formula, fmt) in cartoes {
            let (c0, c1) = (*c0, *c1);
            self.pintar(r, c0, r, c1, &estilo().fundo(clara).cor(APAGADO).tamanho(10.5));
            self.celula(r, c0, &rotulo.to_uppercase(), estilo().borda(&["left"], forte));
            self.mesclar(r, c0, r, c1);
            let valor = estilo().fundo(clara).cor(forte).negrito().tamanho(18.0).formato(*fmt).alinhar("left");
            self.pintar(r + 1, c0, r + 1, c1, &valor);
            self.celula(r + 1, c0, formula, estilo().borda(&["left"], forte));
            self.mesclar(r + 1, c0, r + 1, c1);
        }
        self.altura(r, ALTURA_ROTULO);
        self.altura(r + 1, ALTURA_INDICADOR);
        self.altura(r + 2, ALTURA_RESPIRO);
    }

    fn cabecalho(&mut self, r: u32, c0: u32, titulos: &[&str]) {
        let forte = self.t.forte;
        for (j, t) in titulos.iter().enumerate() {
            let e = estilo().fundo(forte).cor(BRANCO).negrito().alinhar("center").quebrar();
            self.celula(r, c0 + j as u32, t, e);
        }
        self.altura(r, ALTURA_CABECALHO);
    }

    /// Linhas de dados listradas a partir de `r0`; devolve a última linha.
    fn linhas(&mut self, r0: u32, c0: u32, fmts: &[Fmt], dados: &[Vec<String>]) -> u32 {
        let listra = self.t.listra;
        for (i, celulas) in dados.iter().enumerate() {
            let r = r0 + i as u32;
            let fundo = if i % 2 == 1 { listra } else { BRANCO };
            for (j, raw) in celulas.iter().enumerate() {
                let f = fmts.get(j).copied().unwrap_or(Fmt::Texto);
                let e = estilo().fundo(fundo).cor(TEXTO).borda(&["bottom"], LINHA).formato(f);
                self.celula(r, c0 + j as u32, raw, e);
            }
            self.altura(r, ALTURA_LINHA);
        }
        r0 + dados.len().saturating_sub(1) as u32
    }

    /// Linha de total: negrito, fundo claro e um traço forte em cima.
    fn total(&mut self, r: u32, c0: u32, celulas: &[(&str, Fmt)]) {
        let Tema { forte, clara, .. } = self.t;
        for (j, (raw, f)) in celulas.iter().enumerate() {
            let e = estilo().fundo(clara).cor(TEXTO).negrito().borda(&["top"], forte).formato(*f);
            self.celula(r, c0 + j as u32, raw, e);
        }
        self.altura(r, ALTURA_LINHA + 2);
    }

    /// Nota em itálico, apagada, mesclada de A até `ultima`.
    fn nota(&mut self, r: u32, ultima: u32, texto: &str) {
        self.celula(r, 0, texto, estilo().cor(APAGADO).italico().tamanho(10.5));
        self.mesclar(r, 0, r, ultima);
    }

    // ---- formatação condicional (o JSON de `sheet.r`)

    fn regra(&mut self, faixa: (u32, u32, u32, u32), condicao: String) {
        let (r0, c0, r1, c1) = faixa;
        let id = format!("modelo-{}", self.regras.len() + 1);
        self.regras.push(format!(
            r#"{{"id":"{id}","ranges":[{{"r0":{r0},"c0":{c0},"r1":{r1},"c1":{c1}}}],"condition":{condicao}}}"#
        ));
    }

    /// Regra de célula: `op` como o motor nomeia (`equal`, `lessThan`, `formula`…).
    fn se(&mut self, faixa: (u32, u32, u32, u32), op: &str, valor: &str, fundo: Option<&str>, cor: Option<&str>, negrito: bool) {
        let mut estilo = Vec::new();
        if let Some(f) = fundo { estilo.push(format!(r#""bg":"{f}""#)); }
        if let Some(c) = cor { estilo.push(format!(r#""color":"{c}""#)); }
        if negrito { estilo.push(r#""bold":true"#.to_string()); }
        let valores = if valor.is_empty() { String::new() } else { json_texto(valor) };
        self.regra(faixa, format!(
            r#"{{"kind":"cell","op":"{op}","values":[{valores}],"style":{{{}}}}}"#,
            estilo.join(",")
        ));
    }

    /// Texto igual a `valor` ganha a situação (fundo e cor) inteira.
    fn situacao(&mut self, faixa: (u32, u32, u32, u32), valor: &str, (fundo, cor): (&str, &str)) {
        self.se(faixa, "equal", valor, Some(fundo), Some(cor), true);
    }

    fn barra(&mut self, faixa: (u32, u32, u32, u32), cor: &str) {
        self.regra(faixa, format!(r#"{{"kind":"dataBar","color":"{cor}"}}"#));
    }

    fn escala(&mut self, faixa: (u32, u32, u32, u32), pontos: [(f64, &str); 3]) {
        let p = |(v, c): (f64, &str)| format!(r#"{{"type":"number","value":{v},"color":"{c}"}}"#);
        self.regra(faixa, format!(
            r#"{{"kind":"colorScale","min":{},"mid":{},"max":{}}}"#,
            p(pontos[0]), p(pontos[1]), p(pontos[2])
        ));
    }

    fn concluir(self) {
        if !self.mesclas.is_empty() {
            let m: Vec<String> = self.mesclas.iter()
                .map(|[r0, c0, r1, c1]| format!(r#"{{"r0":{r0},"c0":{c0},"r1":{r1},"c1":{c1}}}"#)).collect();
            self.s.mc = Some(format!("[{}]", m.join(",")));
        }
        if !self.regras.is_empty() {
            self.s.r = Some(format!("[{}]", self.regras.join(",")));
        }
    }
}

/// Uma pasta com uma aba, montada pela função.
fn pasta(aba: &str, t: Tema, montar: impl FnOnce(&mut Folha)) -> Workbook {
    let mut w = Workbook::vazia();
    w.criar_aba(aba);
    w.editar(0, |s| {
        let mut f = Folha { s, t, mesclas: Vec::new(), regras: Vec::new() };
        montar(&mut f);
        f.concluir();
    });
    w
}

/// Linha onde começam os indicadores, o cabeçalho e os dados (base zero),
/// iguais em todos os modelos com cartões.
const R_INDICADORES: u32 = 3;
const R_CABECALHO: u32 = 6;
const R_DADOS: u32 = 7;
/// Até onde as fórmulas de resumo olham, para a tabela crescer sem editar fórmula.
const R_FIM: u32 = 499;

// ------------------------------------------------------- modelos

fn orcamento_domestico() -> Workbook {
    pasta("Orçamento", VERDE, |f| {
        f.titulo(4, "Orçamento doméstico",
            "Preencha Previsto e Realizado — diferenças, totais e saldo se calculam sozinhos.");

        // Receitas
        f.cabecalho(R_CABECALHO, 0, &["Receitas", "Previsto", "Realizado", "Diferença", "% do total"]);
        let receitas = [("Salário", "4500", "4500"), ("Renda extra", "800", "650"), ("Outras receitas", "200", "120")];
        let rt = R_DADOS + receitas.len() as u32; // linha do total de receitas
        let dados: Vec<Vec<String>> = receitas.iter().enumerate().map(|(i, (n, p, r))| {
            let x = R_DADOS + i as u32;
            vec![n.to_string(), p.to_string(), r.to_string(),
                format!("={}-{}", a1(x, 2), a1(x, 1)),
                format!("=SE({tot}=0;0;{}/{tot})", a1(x, 2), tot = format!("$C${}", rt + 1))]
        }).collect();
        let ult = f.linhas(R_DADOS, 0, &[Fmt::Texto, Fmt::Moeda, Fmt::Moeda, Fmt::Moeda, Fmt::Pct], &dados);
        let (sb, sc, se) = (format!("=SOMA({})", col(1, R_DADOS, ult)), format!("=SOMA({})", col(2, R_DADOS, ult)),
            format!("=SOMA({})", col(4, R_DADOS, ult)));
        let dif = format!("={}-{}", a1(rt, 2), a1(rt, 1));
        f.total(rt, 0, &[("Total de receitas", Fmt::Texto), (&sb, Fmt::Moeda), (&sc, Fmt::Moeda), (&dif, Fmt::Moeda), (&se, Fmt::Pct)]);
        f.se((R_DADOS, 3, ult, 3), "lessThan", "0", None, Some(RUIM.1), true);
        f.altura(rt + 1, ALTURA_RESPIRO);

        // Despesas
        let rc = rt + 2;
        f.cabecalho(rc, 0, &["Despesas", "Previsto", "Realizado", "Diferença", "% da receita"]);
        let despesas = [
            ("Moradia (aluguel ou condomínio)", "1500", "1500"),
            ("Alimentação", "900", "1010"),
            ("Transporte", "400", "380"),
            ("Contas (luz, água, internet)", "350", "372"),
            ("Saúde", "250", "190"),
            ("Educação", "300", "300"),
            ("Lazer", "200", "260"),
            ("Outras despesas", "150", "90"),
        ];
        let rd = rc + 1;
        let receita_real = format!("${}${}", "C", rt + 1);
        let dados: Vec<Vec<String>> = despesas.iter().enumerate().map(|(i, (n, p, r))| {
            let x = rd + i as u32;
            vec![n.to_string(), p.to_string(), r.to_string(),
                format!("={}-{}", a1(x, 2), a1(x, 1)),
                format!("=SE({receita_real}=0;0;{}/{receita_real})", a1(x, 2))]
        }).collect();
        let ultd = f.linhas(rd, 0, &[Fmt::Texto, Fmt::Moeda, Fmt::Moeda, Fmt::Moeda, Fmt::Pct], &dados);
        let td = ultd + 1;
        let (sb, sc, se) = (format!("=SOMA({})", col(1, rd, ultd)), format!("=SOMA({})", col(2, rd, ultd)),
            format!("=SOMA({})", col(4, rd, ultd)));
        let dif = format!("={}-{}", a1(td, 2), a1(td, 1));
        f.total(td, 0, &[("Total de despesas", Fmt::Texto), (&sb, Fmt::Moeda), (&sc, Fmt::Moeda), (&dif, Fmt::Moeda), (&se, Fmt::Pct)]);
        f.se((rd, 3, ultd, 3), "greaterThan", "0", None, Some(RUIM.1), true);
        f.se((rd, 3, ultd, 3), "lessThan", "0", None, Some(BOM.1), true);
        f.barra((rd, 4, ultd, 4), "#86C5A0");
        f.altura(td + 1, ALTURA_RESPIRO);

        // Saldo
        let rs = td + 2;
        let prev = format!("={}-{}", a1(rt, 1), a1(td, 1));
        let real = format!("={}-{}", a1(rt, 2), a1(td, 2));
        let dif = format!("={}-{}", a1(rs, 2), a1(rs, 1));
        let sit = format!(r#"=SE({}>=0;"Sobrou dinheiro";"Faltou dinheiro")"#, a1(rs, 2));
        f.total(rs, 0, &[("Saldo do mês", Fmt::Texto), (&prev, Fmt::Moeda), (&real, Fmt::Moeda), (&dif, Fmt::Moeda), (&sit, Fmt::Centro)]);
        f.situacao((rs, 4, rs, 4), "Sobrou dinheiro", BOM);
        f.situacao((rs, 4, rs, 4), "Faltou dinheiro", RUIM);
        f.nota(rs + 2, 4, "Dica: uma reserva de emergência costuma cobrir de 3 a 6 meses de despesas.");

        f.indicadores(R_INDICADORES, &[
            (0, 0, "Receitas do mês", format!("={}", a1(rt, 2)), Fmt::Moeda),
            (1, 2, "Despesas do mês", format!("={}", a1(td, 2)), Fmt::Moeda),
            (3, 3, "Saldo", format!("={}", a1(rs, 2)), Fmt::Moeda),
            (4, 4, "Comprometido", format!("=SE({0}=0;0;{1}/{0})", a1(rt, 2), a1(td, 2)), Fmt::Pct),
        ]);
        f.se((R_INDICADORES + 1, 3, R_INDICADORES + 1, 3), "lessThan", "0", None, Some(RUIM.1), true);

        f.larguras(&[270, 125, 125, 125, 140]);
    })
}

fn controle_gastos() -> Workbook {
    let gastos: [(&str, &str, &str, &str); 13] = [
        ("01/10/2026", "Supermercado", "Alimentação", "486"),
        ("02/10/2026", "Gasolina", "Transporte", "250"),
        ("03/10/2026", "Conta de luz", "Moradia", "210"),
        ("04/10/2026", "Cinema", "Lazer", "60"),
        ("05/10/2026", "Farmácia", "Saúde", "95"),
        ("06/10/2026", "Curso de inglês", "Educação", "180"),
        ("07/10/2026", "Restaurante", "Alimentação", "120"),
        ("08/10/2026", "Aplicativo de transporte", "Transporte", "38"),
        ("09/10/2026", "Internet", "Moradia", "120"),
        ("10/10/2026", "Consulta médica", "Saúde", "350"),
        ("11/10/2026", "Livraria", "Educação", "75"),
        ("12/10/2026", "Feira", "Alimentação", "83"),
        ("13/10/2026", "Presente de aniversário", "Outros", "110"),
    ];
    pasta("Gastos", LARANJA, |f| {
        f.titulo(7, "Controle de gastos do mês",
            "Lance cada gasto na tabela da esquerda; o resumo por categoria se atualiza sozinho.");
        let valores = col(3, R_DADOS, R_FIM);

        // Resumo por categoria (à direita)
        f.cabecalho(R_CABECALHO, 5, &["Categoria", "Total", "% do total"]);
        let categorias = ["Alimentação", "Transporte", "Moradia", "Saúde", "Educação", "Lazer", "Outros"];
        let tc = R_DADOS + categorias.len() as u32;
        let total_cat = format!("${}${}", "G", tc + 1);
        let dados: Vec<Vec<String>> = categorias.iter().enumerate().map(|(i, c)| {
            let x = R_DADOS + i as u32;
            vec![c.to_string(),
                format!("=SOMASE({};{};{})", col_abs(2, R_DADOS, R_FIM), a1(x, 5), col_abs(3, R_DADOS, R_FIM)),
                format!("=SE({total_cat}=0;0;{}/{total_cat})", a1(x, 6))]
        }).collect();
        let ult = f.linhas(R_DADOS, 5, &[Fmt::Texto, Fmt::Moeda, Fmt::Pct], &dados);
        let (sg, sh) = (format!("=SOMA({})", col(6, R_DADOS, ult)), format!("=SOMA({})", col(7, R_DADOS, ult)));
        f.total(tc, 5, &[("Total", Fmt::Texto), (&sg, Fmt::Moeda), (&sh, Fmt::Pct)]);
        f.barra((R_DADOS, 7, ult, 7), "#F4A574");

        // Lançamentos (à esquerda), por último: as alturas das linhas são dela
        f.cabecalho(R_CABECALHO, 0, &["Data", "Descrição", "Categoria", "Valor"]);
        let dados: Vec<Vec<String>> = gastos.iter().map(|(d, s, c, v)| linha(&[d, s, c, v])).collect();
        f.linhas(R_DADOS, 0, &[Fmt::Data, Fmt::Texto, Fmt::Centro, Fmt::Moeda], &dados);
        f.se((R_DADOS, 3, R_FIM, 3), "greaterOrEqual", "300", Some(ATENCAO.0), Some(ATENCAO.1), true);
        f.nota(R_DADOS + gastos.len() as u32 + 1, 3, "Gastos de R$ 300 ou mais ficam destacados em amarelo.");

        f.indicadores(R_INDICADORES, &[
            (0, 1, "Total gasto", format!("=SOMA({valores})"), Fmt::Moeda),
            (2, 3, "Lançamentos", format!("=CONT.NÚM({valores})"), Fmt::Inteiro),
            (5, 6, "Maior gasto", format!("=MÁXIMO({valores})"), Fmt::Moeda),
            (7, 7, "Média por gasto", format!("=SE(CONT.NÚM({valores})=0;0;MÉDIA({valores}))"), Fmt::Moeda),
        ]);

        f.larguras(&[105, 230, 130, 115, 28, 140, 125, 110]);
    })
}

fn fluxo_caixa() -> Workbook {
    let meses: [(&str, &str, &str); 12] = [
        ("Janeiro", "8000", "6100"),
        ("Fevereiro", "7800", "6300"),
        ("Março", "8200", "6900"),
        ("Abril", "7900", "8100"),
        ("Maio", "8100", "6500"),
        ("Junho", "9000", "7400"),
        ("Julho", "8000", "8600"),
        ("Agosto", "8300", "7200"),
        ("Setembro", "8100", "6700"),
        ("Outubro", "8400", "7000"),
        ("Novembro", "8200", "7600"),
        ("Dezembro", "9500", "8900"),
    ];
    pasta("Fluxo de caixa", AZUL, |f| {
        f.titulo(5, "Fluxo de caixa anual",
            "Informe o saldo inicial de janeiro e as entradas e saídas de cada mês.");
        f.cabecalho(R_CABECALHO, 0, &["Mês", "Saldo inicial", "Entradas", "Saídas", "Resultado do mês", "Saldo acumulado"]);
        let dados: Vec<Vec<String>> = meses.iter().enumerate().map(|(i, (m, e, s))| {
            let x = R_DADOS + i as u32;
            let inicial = if i == 0 { "5000".to_string() } else { format!("={}", a1(x - 1, 5)) };
            vec![m.to_string(), inicial, e.to_string(), s.to_string(),
                format!("={}-{}", a1(x, 2), a1(x, 3)),
                format!("={}+{}", a1(x, 1), a1(x, 4))]
        }).collect();
        let ult = f.linhas(R_DADOS, 0, &[Fmt::Texto, Fmt::Moeda, Fmt::Moeda, Fmt::Moeda, Fmt::Moeda, Fmt::Moeda], &dados);
        let t = ult + 1;
        let (sc, sd, se, sf) = (format!("=SOMA({})", col(2, R_DADOS, ult)), format!("=SOMA({})", col(3, R_DADOS, ult)),
            format!("=SOMA({})", col(4, R_DADOS, ult)), format!("={}", a1(ult, 5)));
        f.total(t, 0, &[("Total do ano", Fmt::Texto), ("", Fmt::Texto), (&sc, Fmt::Moeda), (&sd, Fmt::Moeda), (&se, Fmt::Moeda), (&sf, Fmt::Moeda)]);
        f.se((R_DADOS, 4, ult, 4), "lessThan", "0", Some(RUIM.0), Some(RUIM.1), true);
        f.se((R_DADOS, 4, ult, 4), "greaterOrEqual", "0", None, Some(BOM.1), false);
        f.barra((R_DADOS, 5, ult, 5), "#8FB0F5");
        f.nota(t + 2, 5, "Meses em vermelho gastaram mais do que entrou.");

        f.indicadores(R_INDICADORES, &[
            (0, 1, "Entradas no ano", format!("={}", a1(t, 2)), Fmt::Moeda),
            (2, 2, "Saídas no ano", format!("={}", a1(t, 3)), Fmt::Moeda),
            (3, 4, "Saldo final", format!("={}", a1(ult, 5)), Fmt::Moeda),
            (5, 5, "Meses no vermelho", format!(r#"=CONT.SE({};"<0")"#, col(4, R_DADOS, ult)), Fmt::Inteiro),
        ]);
        f.se((R_INDICADORES + 1, 5, R_INDICADORES + 1, 5), "greaterThan", "0", None, Some(RUIM.1), true);

        f.larguras(&[130, 135, 135, 135, 150, 150]);
    })
}

fn lista_tarefas() -> Workbook {
    let tarefas: [(&str, &str, &str, &str, &str); 10] = [
        ("Definir escopo do projeto", "Ana", "05/10/2026", "Alta", "Concluída"),
        ("Levantar requisitos", "Bruno", "09/10/2026", "Alta", "Concluída"),
        ("Montar cronograma", "Ana", "15/10/2026", "Média", "Em andamento"),
        ("Comprar equipamentos", "Carla", "20/10/2026", "Média", "A fazer"),
        ("Contratar fornecedor", "Bruno", "25/10/2026", "Alta", "Em andamento"),
        ("Treinar equipe", "Carla", "30/10/2026", "Baixa", "A fazer"),
        ("Revisar orçamento", "Ana", "12/10/2026", "Alta", "A fazer"),
        ("Escrever relatório mensal", "Diego", "31/10/2026", "Média", "A fazer"),
        ("Reunião de acompanhamento", "Diego", "14/10/2026", "Baixa", "Em andamento"),
        ("Fechar contratos", "Bruno", "28/10/2026", "Alta", "A fazer"),
    ];
    pasta("Tarefas", ROXO, |f| {
        f.titulo(6, "Lista de tarefas",
            "Atualize o Status; a situação e os dias restantes se calculam pela data de hoje.");
        f.cabecalho(R_CABECALHO, 0, &["Tarefa", "Responsável", "Prazo", "Prioridade", "Status", "Situação", "Dias restantes"]);
        let dados: Vec<Vec<String>> = tarefas.iter().enumerate().map(|(i, (t, r, p, pr, st))| {
            let x = R_DADOS + i as u32;
            let (c, e) = (a1(x, 2), a1(x, 4));
            vec![t.to_string(), r.to_string(), p.to_string(), pr.to_string(), st.to_string(),
                format!(r#"=SE({e}="Concluída";"Concluída";SE({c}<HOJE();"Atrasada";"No prazo"))"#),
                format!(r#"=SE({e}="Concluída";"";{c}-HOJE())"#)]
        }).collect();
        f.linhas(R_DADOS, 0, &[Fmt::Texto, Fmt::Texto, Fmt::Data, Fmt::Centro, Fmt::Centro, Fmt::Centro, Fmt::Inteiro], &dados);
        let (status, prioridade, situacao, dias) =
            ((R_DADOS, 4, R_FIM, 4), (R_DADOS, 3, R_FIM, 3), (R_DADOS, 5, R_FIM, 5), (R_DADOS, 6, R_FIM, 6));
        f.situacao(status, "Concluída", BOM);
        f.situacao(status, "Em andamento", ATENCAO);
        f.situacao(status, "A fazer", NEUTRO);
        f.se(prioridade, "equal", "Alta", None, Some(RUIM.1), true);
        f.se(prioridade, "equal", "Média", None, Some(ATENCAO.1), true);
        f.se(prioridade, "equal", "Baixa", None, Some(APAGADO), false);
        f.situacao(situacao, "Atrasada", RUIM);
        f.se(situacao, "equal", "No prazo", None, Some(BOM.1), false);
        f.se(situacao, "equal", "Concluída", None, Some(APAGADO), false);
        f.se(dias, "lessThan", "0", None, Some(RUIM.1), true);

        let st = col(4, R_DADOS, R_FIM);
        f.indicadores(R_INDICADORES, &[
            (0, 0, "Tarefas", format!("=CONT.VALORES({})", col(0, R_DADOS, R_FIM)), Fmt::Inteiro),
            (1, 2, "Concluídas", format!(r#"=CONT.SE({st};"Concluída")"#), Fmt::Inteiro),
            (3, 4, "Em andamento", format!(r#"=CONT.SE({st};"Em andamento")"#), Fmt::Inteiro),
            (5, 6, "Atrasadas", format!(r#"=CONT.SE({};"Atrasada")"#, col(5, R_DADOS, R_FIM)), Fmt::Inteiro),
        ]);
        f.se((R_INDICADORES + 1, 5, R_INDICADORES + 1, 5), "greaterThan", "0", None, Some(RUIM.1), true);

        f.larguras(&[270, 125, 110, 105, 130, 115, 115]);
    })
}

fn inventario() -> Workbook {
    // produto, código, quantidade, estoque mínimo, custo unitário
    let itens: [(&str, &str, &str, &str, &str); 12] = [
        ("Papel A4 (resma)", "PA-001", "40", "20", "24"),
        ("Caneta esferográfica", "CA-002", "15", "30", "2"),
        ("Grampeador", "GR-003", "8", "5", "35"),
        ("Caixa de grampos", "GR-004", "12", "10", "6"),
        ("Pasta suspensa", "PS-005", "4", "15", "9"),
        ("Toner para impressora", "TN-006", "2", "3", "380"),
        ("Marcador de texto", "MA-007", "25", "10", "5"),
        ("Fita adesiva", "FA-008", "6", "12", "8"),
        ("Envelope saco", "EN-009", "300", "100", "1"),
        ("Calculadora", "CL-010", "5", "2", "60"),
        ("Bloco de notas", "BN-011", "0", "10", "7"),
        ("Clipes", "CP-012", "80", "50", "3"),
    ];
    pasta("Estoque", PETROLEO, |f| {
        f.titulo(8, "Controle de estoque",
            "Atualize as quantidades; alerta, reposição e custos se calculam sozinhos.");
        f.cabecalho(R_CABECALHO, 0, &["Produto", "Código", "Quantidade", "Estoque mínimo", "Custo unitário",
            "Total em estoque", "Alerta", "Repor", "Custo da reposição"]);
        let dados: Vec<Vec<String>> = itens.iter().enumerate().map(|(i, (p, c, q, m, u))| {
            let x = R_DADOS + i as u32;
            let (qa, ma, ua, ha) = (a1(x, 2), a1(x, 3), a1(x, 4), a1(x, 7));
            vec![p.to_string(), c.to_string(), q.to_string(), m.to_string(), u.to_string(),
                format!("={qa}*{ua}"),
                format!(r#"=SE({qa}<{ma};"Repor";"OK")"#),
                format!("=SE({qa}<{ma};{ma}-{qa};0)"),
                format!("={ha}*{ua}")]
        }).collect();
        let ult = f.linhas(R_DADOS, 0, &[Fmt::Texto, Fmt::Centro, Fmt::Inteiro, Fmt::Inteiro, Fmt::Moeda,
            Fmt::Moeda, Fmt::Centro, Fmt::Inteiro, Fmt::Moeda], &dados);
        let t = ult + 1;
        let soma = |c| format!("=SOMA({})", col(c, R_DADOS, ult));
        let (sc, sf, sh, si) = (soma(2), soma(5), soma(7), soma(8));
        f.total(t, 0, &[("Total", Fmt::Texto), ("", Fmt::Texto), (&sc, Fmt::Inteiro), ("", Fmt::Texto), ("", Fmt::Texto),
            (&sf, Fmt::Moeda), ("", Fmt::Texto), (&sh, Fmt::Inteiro), (&si, Fmt::Moeda)]);
        f.situacao((R_DADOS, 6, ult, 6), "Repor", RUIM);
        f.situacao((R_DADOS, 6, ult, 6), "OK", BOM);
        f.se((R_DADOS, 7, ult, 7), "greaterThan", "0", None, Some(ATENCAO.1), true);
        f.barra((R_DADOS, 5, ult, 5), "#7CC8BF");

        f.indicadores(R_INDICADORES, &[
            (0, 1, "Produtos", format!("=CONT.VALORES({})", col(0, R_DADOS, ult)), Fmt::Inteiro),
            (2, 4, "Valor em estoque", format!("={}", a1(t, 5)), Fmt::Moeda),
            (5, 6, "Itens para repor", format!(r#"=CONT.SE({};"Repor")"#, col(6, R_DADOS, ult)), Fmt::Inteiro),
            (7, 8, "Custo da reposição", format!("={}", a1(t, 8)), Fmt::Moeda),
        ]);
        f.se((R_INDICADORES + 1, 5, R_INDICADORES + 1, 5), "greaterThan", "0", None, Some(RUIM.1), true);

        f.larguras(&[220, 90, 105, 120, 115, 130, 95, 85, 140]);
    })
}

fn notas_escolares() -> Workbook {
    // aluno, matrícula, prova 1, prova 2, trabalho
    let alunos: [(&str, &str, &str, &str, &str); 15] = [
        ("Ana Beatriz Souza", "2026001", "8", "9", "7"),
        ("Bruno Lima", "2026002", "5", "6", "6"),
        ("Carla Mendes", "2026003", "9", "8", "10"),
        ("Daniel Rocha", "2026004", "4", "5", "6"),
        ("Eduarda Alves", "2026005", "7", "7", "8"),
        ("Felipe Costa", "2026006", "6", "4", "5"),
        ("Gabriela Nunes", "2026007", "10", "9", "9"),
        ("Henrique Dias", "2026008", "3", "5", "4"),
        ("Isabela Ramos", "2026009", "8", "6", "7"),
        ("João Pedro Melo", "2026010", "7", "8", "6"),
        ("Larissa Freitas", "2026011", "5", "7", "8"),
        ("Lucas Teixeira", "2026012", "6", "6", "5"),
        ("Mariana Gomes", "2026013", "9", "10", "9"),
        ("Nicolas Pinto", "2026014", "2", "4", "5"),
        ("Olívia Barros", "2026015", "7", "6", "8"),
    ];
    pasta("Notas", INDIGO, |f| {
        f.titulo(6, "Boletim de notas", "Notas de 0 a 10. Aprovado com média 7 ou mais.");
        f.cabecalho(R_CABECALHO, 0, &["Aluno", "Matrícula", "Prova 1", "Prova 2", "Trabalho", "Média", "Situação"]);
        let dados: Vec<Vec<String>> = alunos.iter().enumerate().map(|(i, (a, m, p1, p2, t))| {
            let x = R_DADOS + i as u32;
            vec![a.to_string(), m.to_string(), p1.to_string(), p2.to_string(), t.to_string(),
                format!("=MÉDIA({}:{})", a1(x, 2), a1(x, 4)),
                format!(r#"=SE({}>=7;"Aprovado";"Reprovado")"#, a1(x, 5))]
        }).collect();
        let ult = f.linhas(R_DADOS, 0, &[Fmt::Texto, Fmt::Centro, Fmt::Inteiro, Fmt::Inteiro, Fmt::Inteiro, Fmt::Nota, Fmt::Centro], &dados);
        let t = ult + 1;
        let media = |c| format!("=MÉDIA({})", col(c, R_DADOS, ult));
        let (mc, md, me, mf) = (media(2), media(3), media(4), media(5));
        f.total(t, 0, &[("Média da turma", Fmt::Texto), ("", Fmt::Texto), (&mc, Fmt::Nota), (&md, Fmt::Nota),
            (&me, Fmt::Nota), (&mf, Fmt::Nota), ("", Fmt::Texto)]);
        f.escala((R_DADOS, 2, ult, 5), [(0.0, "#F8A5A5"), (7.0, "#FDE68A"), (10.0, "#86D19B")]);
        f.situacao((R_DADOS, 6, ult, 6), "Aprovado", BOM);
        f.situacao((R_DADOS, 6, ult, 6), "Reprovado", RUIM);

        let sit = col(6, R_DADOS, ult);
        f.indicadores(R_INDICADORES, &[
            (0, 0, "Média da turma", format!("={}", a1(t, 5)), Fmt::Nota),
            (1, 2, "Aprovados", format!(r#"=CONT.SE({sit};"Aprovado")"#), Fmt::Inteiro),
            (3, 4, "Reprovados", format!(r#"=CONT.SE({sit};"Reprovado")"#), Fmt::Inteiro),
            (5, 6, "Aprovação", format!(r#"=SE(CONT.VALORES({sit})=0;0;CONT.SE({sit};"Aprovado")/CONT.VALORES({sit}))"#), Fmt::Pct),
        ]);
        f.se((R_INDICADORES + 1, 3, R_INDICADORES + 1, 3), "greaterThan", "0", None, Some(RUIM.1), true);
        f.nota(t + 2, 6, "Média = média simples das três avaliações.");

        f.larguras(&[220, 110, 95, 95, 95, 95, 120]);
    })
}

/// Nome do mês de hoje e o ano, em fórmula (o título do calendário acompanha a data).
const MES_ANO: &str = r#"=ESCOLHER(MÊS(HOJE());"Janeiro";"Fevereiro";"Março";"Abril";"Maio";"Junho";"Julho";"Agosto";"Setembro";"Outubro";"Novembro";"Dezembro")&" de "&ANO(HOJE())"#;

fn calendario_mensal() -> Workbook {
    pasta("Calendário", ROSA, |f| {
        let Tema { forte, clara, listra } = f.t;
        f.titulo(6, MES_ANO, "O calendário acompanha a data de hoje — o dia atual fica destacado.");
        let rc = 3;
        let dias = ["Domingo", "Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado"];
        f.cabecalho(rc, 0, &dias);
        // Dias vazios antes do dia 1 (domingo = 0).
        let antes = "MOD(DATA(ANO(HOJE());MÊS(HOJE());1)-1;7)";
        for semana in 0..6u32 {
            let r = rc + 1 + semana;
            for dia in 0..7u32 {
                let k = semana * 7 + dia + 1;
                let data = format!("DATA(ANO(HOJE());MÊS(HOJE());{k}-{antes})");
                let formula = format!(r#"=SE(MÊS({data})=MÊS(HOJE());DIA({data});"")"#);
                let fim_de_semana = dia == 0 || dia == 6;
                let e = estilo()
                    .fundo(if fim_de_semana { listra } else { BRANCO })
                    .cor(if fim_de_semana { forte } else { TEXTO })
                    .negrito().tamanho(15.0).alinhar("left").topo()
                    .borda(&["top", "right", "bottom", "left"], LINHA)
                    .formato(Fmt::Texto);
                f.celula(r, dia, &formula, e);
            }
            f.altura(r, 76);
        }
        let grade = (rc + 1, 0, rc + 6, 6);
        f.se(grade, "formula", &format!("={}=DIA(HOJE())", a1(rc + 1, 0)), Some(forte), Some(BRANCO), true);

        // Anotações do mês
        let ra = rc + 8;
        f.altura(rc + 7, ALTURA_RESPIRO);
        f.pintar(ra, 0, ra, 6, &estilo().fundo(clara).cor(forte).negrito());
        f.celula(ra, 0, "Anotações do mês", estilo());
        f.mesclar(ra, 0, ra, 6);
        f.altura(ra, ALTURA_CABECALHO);
        for i in 1..=5u32 {
            f.pintar(ra + i, 0, ra + i, 6, &estilo().fundo(BRANCO).cor(TEXTO).borda(&["bottom"], LINHA));
            f.mesclar(ra + i, 0, ra + i, 6);
            f.altura(ra + i, ALTURA_LINHA + 3);
        }

        f.larguras(&[125, 125, 125, 125, 125, 125, 125]);
    })
}

fn agenda_semanal() -> Workbook {
    pasta("Agenda", CIANO, |f| {
        let Tema { forte, clara, listra } = f.t;
        f.titulo(7, "Agenda da semana", "De segunda a domingo da semana atual; o dia de hoje fica em destaque.");
        let (rd, rn) = (3u32, 4u32); // linha dos dias da semana e das datas
        let dias = ["Segunda", "Terça", "Quarta", "Quinta", "Sexta", "Sábado", "Domingo"];
        f.cabecalho(rd, 0, &["Horário"]);
        f.celula(rn, 0, "", estilo().fundo(forte));
        for (j, d) in dias.iter().enumerate() {
            let c = j as u32 + 1;
            f.celula(rd, c, d, estilo().fundo(forte).cor(BRANCO).negrito().alinhar("center"));
            // Segunda-feira desta semana + j dias.
            let data = format!("=HOJE()-MOD(HOJE()-2;7)+{j}");
            f.celula(rn, c, &data, estilo().fundo(forte).cor(clara).alinhar("center").formato(Fmt::Data));
        }
        f.mesclar(rd, 0, rn, 0);
        f.altura(rd, 26);
        f.altura(rn, 22);
        let horas: Vec<String> = (7..=20).map(|h| format!("{h:02}:00")).collect();
        for (i, h) in horas.iter().enumerate() {
            let r = rn + 1 + i as u32;
            let fundo = if i % 2 == 1 { listra } else { BRANCO };
            f.celula(r, 0, h, estilo().fundo(clara).cor(forte).negrito().alinhar("center")
                .borda(&["bottom"], LINHA));
            f.pintar(r, 1, r, 7, &estilo().fundo(fundo).cor(TEXTO).tamanho(11.0)
                .borda(&["bottom", "left"], LINHA));
            f.altura(r, 34);
        }
        let ultima = rn + horas.len() as u32;
        // Compromissos de exemplo, com cor por tipo.
        let compromissos: [(u32, u32, &str, (&str, &str)); 7] = [
            (2, 1, "Reunião de equipe", ("#DBEAFE", "#1E40AF")),
            (4, 3, "Almoço com cliente", ("#FCE7F3", "#9D174D")),
            (6, 2, "Dentista", ("#FEF3C7", "#92400E")),
            (3, 5, "Entrega do relatório", ("#FEE2E2", "#B91C1C")),
            (11, 1, "Academia", ("#DCFCE7", "#166534")),
            (11, 3, "Academia", ("#DCFCE7", "#166534")),
            (4, 6, "Feira", ("#EDE9FE", "#5B21B6")),
        ];
        for (hora, dia, texto, (fundo, cor)) in compromissos {
            let r = rn + 1 + hora;
            f.celula(r, dia, texto, estilo().fundo(fundo).cor(cor).negrito()
                .borda(&["left"], cor));
        }
        // Coluna de hoje: data em destaque no cabeçalho.
        f.se((rn, 1, rn, 7), "formula", &format!("={}=HOJE()", a1(rn, 1)), Some(BRANCO), Some(forte), true);
        f.nota(ultima + 2, 7, "Escreva os compromissos nas células; pinte com Formatar → Cor de preenchimento.");

        f.larguras(&[85, 135, 135, 135, 135, 135, 135, 135]);
    })
}

fn vendas() -> Workbook {
    let vendedores = ["Ana Souza", "Bruno Lima", "Carla Mendes", "Diego Rocha"];
    let regioes = ["Norte", "Nordeste", "Centro-Oeste", "Sudeste", "Sul"];
    let produtos: [(&str, u32); 5] = [("Notebook", 3500), ("Monitor", 1200), ("Impressora", 800), ("Teclado", 150), ("Mouse", 60)];
    let n = 30u32;

    let mut w = pasta("Vendas", CIANO, |f| {
        f.titulo(9, "Relatório de vendas", "Vendas de outubro — os resumos e indicadores acompanham a tabela.");
        let total = col_abs(6, R_DADOS, R_FIM);

        // Resumos à direita
        let resumo = |f: &mut Folha, r: u32, titulo: &str, nomes: &[&str], criterio: u32| -> (u32, u32) {
            f.cabecalho(r, 8, &[titulo, "Valor"]);
            let dados: Vec<Vec<String>> = nomes.iter().enumerate().map(|(i, nome)| {
                let x = r + 1 + i as u32;
                vec![nome.to_string(), format!("=SOMASE({};{};{total})", col_abs(criterio, R_DADOS, R_FIM), a1(x, 8))]
            }).collect();
            let ult = f.linhas(r + 1, 8, &[Fmt::Texto, Fmt::Moeda], &dados);
            let soma = format!("=SOMA({})", col(9, r + 1, ult));
            f.total(ult + 1, 8, &[("Total", Fmt::Texto), (&soma, Fmt::Moeda)]);
            f.barra((r + 1, 9, ult, 9), "#7FC4D6");
            (r + 1, ult)
        };
        resumo(f, R_CABECALHO, "Por região", &regioes, 2);
        let (v0, v1) = resumo(f, R_CABECALHO + regioes.len() as u32 + 3, "Por vendedor", &vendedores, 1);

        f.cabecalho(R_CABECALHO, 0, &["Data", "Vendedor", "Região", "Produto", "Quantidade", "Valor unitário", "Valor total"]);
        let dados: Vec<Vec<String>> = (0..n as usize).map(|i| {
            let x = R_DADOS + i as u32;
            let (produto, preco) = produtos[(i * 5) % produtos.len()];
            vec![format!("{:02}/10/2026", 1 + (i * 3) % 28), vendedores[i % vendedores.len()].to_string(),
                regioes[(i * 7) % regioes.len()].to_string(), produto.to_string(),
                format!("{}", 1 + (i * 11) % 6), preco.to_string(),
                format!("={}*{}", a1(x, 4), a1(x, 5))]
        }).collect();
        let ult = f.linhas(R_DADOS, 0, &[Fmt::Data, Fmt::Texto, Fmt::Centro, Fmt::Texto, Fmt::Inteiro, Fmt::Moeda, Fmt::Moeda], &dados);
        f.se((R_DADOS, 6, ult, 6), "aboveAverage", "", None, Some(BOM.1), true);

        let (nomes, valores) = (col(8, v0, v1), col(9, v0, v1));
        f.indicadores(R_INDICADORES, &[
            (0, 1, "Faturamento", format!("=SOMA({total})"), Fmt::Moeda),
            (2, 3, "Vendas", format!("=CONT.NÚM({total})"), Fmt::Inteiro),
            (4, 6, "Ticket médio", format!("=SE(CONT.NÚM({total})=0;0;MÉDIA({total}))"), Fmt::Moeda),
            (8, 9, "Melhor vendedor", format!("=ÍNDICE({nomes};CORRESP(MÁXIMO({valores});{valores};0))"), Fmt::Texto),
        ]);

        f.larguras(&[105, 140, 130, 120, 105, 125, 130, 28, 150, 135]);
    });
    // Intervalo nomeado da tabela de vendas (com o cabeçalho), para a tabela dinâmica.
    w.definir_nome("TabelaVendas", Rect::new(R_CABECALHO, 0, R_DADOS + n - 1, 6));
    w
}

mod queda_tensao;

/// Linhas e colunas congeladas por aba (índice, linhas, colunas) ao abrir o modelo `id`.
pub fn congelamento(id: &str) -> &'static [(usize, u32, u32)] {
    match id {
        "queda-tensao" => queda_tensao::CONGELAR,
        _ => &[],
    }
}

#[cfg(test)]
mod testes;

