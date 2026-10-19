//! Planilha de amostra de ouro: uma aba por categoria de função (lida do
//! REGISTRO do motor), uma aba de gráficos (um de cada tipo) e uma aba de
//! extras (formatos, comentário, validação, condicional, nomes, mesclagem,
//! recuo). Grava o `.fshz` e o `.xlsx` com os gravadores do próprio projeto,
//! reabre o `.xlsx` com o leitor do projeto e compara fórmula por fórmula.
//!
//! Uso (a partir de `feathersheet/crates/feathersheet-io`):
//!   systemd-run --user --scope -p MemoryMax=8G cargo run --release \
//!     --example amostra_ouro -- [pasta-de-saida]
//!
//! A tabela de funções (categoria, argumentos em pt-BR e o valor que o
//! Excel daria) mora em `amostra_ouro_funcoes.tsv`, ao lado deste arquivo.
//! O programa confere que TODA função do registro aparece nela.

use std::collections::{BTreeMap, BTreeSet};
use std::fmt::Write as _;
use std::path::PathBuf;

use feathersheet_core::celulas::estilo_parcial::{Campo, EstiloParcial};
use feathersheet_core::celulas::locale::{set_locale_config, LocaleConfig};
use feathersheet_core::celulas::refs::{col_name, parse_cell_ref, Rect};
use feathersheet_core::celulas::values::Value;
use feathersheet_core::funcoes::function_help::CATEGORIES;
use feathersheet_core::funcoes::functions as reg;
use feathersheet_core::grafico::chart::{extract_chart_data, ChartConfig, ChartRange, ChartType};
use feathersheet_core::planilha::sheet::comentarios::Comentario;
use feathersheet_core::planilha::workbook::Workbook;
use feathersheet_io::grafico_migrado::renderizar_dados;
use serde_json::json;

const TSV: &str = include_str!("amostra_ouro_funcoes.tsv");

/// Abas de função, na ordem em que aparecem no arquivo.
const ABAS: [&str; 12] = [
    "Matemática", "Estatística", "Compatibilidade", "Texto", "Data e hora", "Lógica",
    "Pesquisa e referência", "Financeira", "Engenharia", "Matriz dinâmica", "Banco de dados",
    "Fonte de dados",
];

/// Funções que derramam e o registro não marca como dinâmicas.
const DERRAMAM_A_MAIS: [&str; 2] = ["MAP", "TEXTSPLIT"];

struct Linha {
    aba: String,
    canon: String,
    args: String,
    esperado: String,
    fmt: String,
}

fn derrama(l: &Linha) -> bool {
    l.fmt != "so_texto" && (reg::is_dynamic(&l.canon) || DERRAMAM_A_MAIS.contains(&l.canon.as_str()))
}

fn ler_tsv() -> Vec<Linha> {
    TSV.lines()
        .filter(|l| !l.trim().is_empty())
        .map(|l| {
            let c: Vec<&str> = l.split('\t').collect();
            Linha {
                aba: c[0].into(),
                canon: c[1].into(),
                args: c.get(2).copied().unwrap_or("").into(),
                esperado: c.get(3).copied().unwrap_or("").into(),
                fmt: c.get(4).copied().unwrap_or("").into(),
            }
        })
        .collect()
}

/// O nome em português de uma função: o primeiro apelido da tabela do motor
/// (o mesmo que a pessoa digitaria), ou o próprio nome quando não há.
fn nome_pt(canon: &str) -> String {
    reg::aliases()
        .iter()
        .find(|(a, r)| *r == canon && *a != canon)
        .map(|(a, _)| (*a).to_string())
        .unwrap_or_else(|| canon.to_string())
}

fn a1(nome: &str) -> (u32, u32) {
    parse_cell_ref(nome).unwrap_or_else(|| panic!("referência inválida: {nome}"))
}

struct Ctx {
    wb: Workbook,
}

impl Ctx {
    fn put(&mut self, aba: usize, ref_a1: &str, texto: &str) {
        let (r, c) = a1(ref_a1);
        self.put_rc(aba, r, c, texto);
    }
    fn put_rc(&mut self, aba: usize, r: u32, c: u32, texto: &str) {
        self.wb.escrever_celula(aba, r, c, texto.to_string());
    }
    /// Texto que NÃO pode ser reinterpretado (começa com "=", parece número,
    /// data, lógico): vai como fórmula que devolve o literal, `="..."`.
    fn put_texto(&mut self, aba: usize, r: u32, c: u32, texto: &str) {
        let s = texto.trim();
        let parece_numero = !s.is_empty()
            && s.chars().all(|ch| ch.is_ascii_digit() || matches!(ch, '.' | ',' | '-' | '+' | 'E' | 'e' | '%' | '/' | ':' | ' '));
        let perigoso = parece_numero
            || s.starts_with('=')
            || s.starts_with('+')
            || s.starts_with('-')
            || s.starts_with('@')
            || s.eq_ignore_ascii_case("verdadeiro")
            || s.eq_ignore_ascii_case("falso");
        if perigoso {
            let q = format!("=\"{}\"", texto.replace('"', "\"\""));
            self.put_rc(aba, r, c, &q);
        } else {
            self.put_rc(aba, r, c, texto);
        }
    }
    fn estilo(&mut self, aba: usize, area: &str, campos: &[(&str, Campo)]) {
        let rect = if area.contains(':') {
            feathersheet_core::celulas::refs::parse_range_ref(area).expect("área")
        } else {
            let (r, c) = a1(area);
            Rect::cell(r, c)
        };
        let mut e = EstiloParcial::new();
        for (k, v) in campos {
            e.insert((*k).to_string(), v.clone());
        }
        self.wb.editar(aba, |s| s.set_style(&rect, &e));
    }
    fn largura(&mut self, aba: usize, col: u32, px: u32) {
        self.wb.editar(aba, |s| {
            s.definir_dimensao(true, col, px);
        });
    }
}

fn t(s: &str) -> Campo {
    Campo::Texto(s.into())
}
fn b(v: bool) -> Campo {
    Campo::Logico(v)
}

/// A área de dados de apoio (colunas M em diante), igual em toda aba de função.
fn bloco_de_apoio(c: &mut Ctx, aba: usize) {
    c.put(aba, "M1", "DADOS DE APOIO (argumentos das fórmulas)");
    // Números A e B, texto, categoria e datas.
    let heads = [("N1", "Núm A"), ("O1", "Núm B"), ("P1", "Texto"), ("Q1", "Categoria"), ("R1", "Datas")];
    for (r, h) in heads {
        c.put(aba, r, h);
    }
    let a = [2, 4, 4, 5, 7, 9, 10, 12, 15, 18];
    let bb = [3, 5, 6, 8, 9, 12, 13, 15, 20, 21];
    let tx = ["Ana", "Bruno", "Carla", "Daniel", "Eva", "Fábio", "Gabi", "Hugo", "Iara", "João"];
    let ca = ["A", "B", "A", "C", "B", "A", "B", "C", "A", "B"];
    for i in 0..10u32 {
        let r = i + 1;
        c.put_rc(aba, r, 13, &a[i as usize].to_string());
        c.put_rc(aba, r, 14, &bb[i as usize].to_string());
        c.put_rc(aba, r, 15, tx[i as usize]);
        c.put_rc(aba, r, 16, ca[i as usize]);
        if i == 0 {
            c.put_rc(aba, r, 17, "=DATA(2026;1;15)");
        } else {
            c.put_rc(aba, r, 17, &format!("=R{}+30", r));
        }
    }
    c.estilo(aba, "R2:R11", &[("numFmt", t("dd/mm/yyyy"))]);
    // Tabela de pesquisa (vertical) e horizontal.
    for (r, h) in [("T1", "Cód"), ("U1", "Produto"), ("V1", "Preço")] {
        c.put(aba, r, h);
    }
    let prod = [("Maçã", "3,5"), ("Banana", "2,2"), ("Cereja", "8"), ("Damasco", "12,9"), ("Figo", "7,5")];
    for (i, (n, p)) in prod.iter().enumerate() {
        let r = i as u32 + 1;
        c.put_rc(aba, r, 19, &(i + 1).to_string());
        c.put_rc(aba, r, 20, n);
        c.put_rc(aba, r, 21, p);
        c.put_rc(aba, 0, 23 + i as u32, &(i + 1).to_string());
        c.put_rc(aba, 1, 23 + i as u32, n);
        c.put_rc(aba, 2, 23 + i as u32, p);
    }
    // Banco de dados e critérios.
    for (i, h) in ["Árvore", "Altura", "Idade", "Rendimento", "Lucro"].iter().enumerate() {
        c.put_rc(aba, 0, 29 + i as u32, h);
    }
    let db = [
        ("Maçã", "18", "20", "14", "105"),
        ("Pêra", "12", "12", "10", "96"),
        ("Cereja", "13", "14", "9", "105"),
        ("Maçã", "14", "15", "10", "75"),
        ("Pêra", "9", "8", "8", "76,8"),
        ("Maçã", "8", "9", "6", "45"),
    ];
    for (i, row) in db.iter().enumerate() {
        let r = i as u32 + 1;
        for (j, v) in [row.0, row.1, row.2, row.3, row.4].iter().enumerate() {
            c.put_rc(aba, r, 29 + j as u32, v);
        }
    }
    c.put(aba, "AJ1", "Árvore");
    c.put(aba, "AK1", "Altura");
    c.put(aba, "AJ2", "Maçã");
    c.put(aba, "AK2", ">10");
    c.put(aba, "AM1", "Árvore");
    c.put(aba, "AM2", "Cereja");
    // Fluxos de caixa (TIR/MTIR/VPL) e datas (XTIR/XVPL).
    c.put(aba, "AS1", "Fluxo");
    for (i, v) in ["-70000", "12000", "15000", "18000", "21000", "26000"].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 44, v);
    }
    c.put(aba, "AS10", "Valores");
    c.put(aba, "AT10", "Datas");
    let fx = ["-10000", "2750", "4250", "3250", "2750", "500"];
    let dts = [(2026, 1, 1), (2026, 3, 1), (2026, 10, 30), (2027, 2, 15), (2027, 4, 1), (2027, 7, 1)];
    for i in 0..6usize {
        c.put_rc(aba, 10 + i as u32, 44, fx[i]);
        c.put_rc(aba, 10 + i as u32, 45, &format!("=DATA({};{};{})", dts[i].0, dts[i].1, dts[i].2));
    }
    c.estilo(aba, "AT11:AT16", &[("numFmt", t("dd/mm/yyyy"))]);
    // Células especiais: vazia (AZ), erros, texto, número, lógico.
    c.put(aba, "AY1", "AZ1:AZ5 ficam vazias");
    c.put(aba, "BA1", "Especiais");
    c.put(aba, "BA2", "=1/0");
    c.put(aba, "BA3", "=NÃO.DISP()");
    c.put(aba, "BA4", "abc");
    c.put(aba, "BA5", "5");
    c.put(aba, "BA6", "VERDADEIRO");
    // Classes (FREQÜÊNCIA), taxas (VFPLANO), probabilidades (PROB), qui-quadrado.
    c.put(aba, "BC1", "Classes");
    for (i, v) in ["5", "10", "15"].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 54, v);
    }
    c.put(aba, "BE1", "Taxas");
    for (i, v) in ["0,09", "0,11", "0,1"].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 56, v);
    }
    c.put(aba, "BG1", "x");
    c.put(aba, "BH1", "p(x)");
    for (i, (x, p)) in [("1", "0,1"), ("2", "0,2"), ("3", "0,3"), ("4", "0,4")].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 58, x);
        c.put_rc(aba, 1 + i as u32, 59, p);
    }
    c.put(aba, "BJ1", "Observado");
    c.put(aba, "BL1", "Esperado");
    let obs = [("58", "35"), ("11", "25"), ("10", "23")];
    let esp = [("45,35", "47,65"), ("17,56", "18,44"), ("16,09", "16,91")];
    for i in 0..3usize {
        c.put_rc(aba, 1 + i as u32, 61, obs[i].0);
        c.put_rc(aba, 1 + i as u32, 62, obs[i].1);
        c.put_rc(aba, 1 + i as u32, 63, esp[i].0);
        c.put_rc(aba, 1 + i as u32, 64, esp[i].1);
    }
    // Matrizes 2x2.
    c.put(aba, "BN1", "Matriz 1");
    c.put(aba, "BQ1", "Matriz 2");
    for (i, v) in [("1", "2"), ("3", "4")].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 65, v.0);
        c.put_rc(aba, 1 + i as u32, 66, v.1);
    }
    for (i, v) in [("5", "6"), ("7", "8")].iter().enumerate() {
        c.put_rc(aba, 1 + i as u32, 68, v.0);
        c.put_rc(aba, 1 + i as u32, 69, v.1);
    }
    // Estilo do bloco: cabeçalhos em negrito, cinza.
    for h in [
        "M1", "N1:R1", "T1:V1", "AD1:AH1", "AJ1:AK1", "AM1", "AS1", "AS10:AT10", "BA1", "BC1", "BE1", "BG1:BH1",
        "BJ1", "BL1", "BN1", "BQ1", "X1:AB1",
    ] {
        c.estilo(aba, h, &[("bold", b(true)), ("color", t("#555555"))]);
    }
    c.largura(aba, 12, 230);
    for col in 13..70u32 {
        c.largura(aba, col, 82);
    }
}

fn escreve_cabecalho(c: &mut Ctx, aba: usize) {
    for (i, h) in ["Função (pt)", "Nome inglês", "Fórmula (texto)", "Resultado", "Esperado (Excel)", "Confere"]
        .iter()
        .enumerate()
    {
        c.put_rc(aba, 0, i as u32, h);
    }
    c.estilo(aba, "A1:F1", &[("bold", b(true)), ("bg", t("#1f4e79")), ("color", t("#ffffff"))]);
    for (col, px) in [(0u32, 190u32), (1, 190), (2, 400), (3, 150), (4, 140), (5, 100)] {
        c.largura(aba, col, px);
    }
}

#[derive(Default)]
struct Relatorio {
    por_aba: BTreeMap<String, usize>,
    ok: usize,
    diferente: Vec<String>,
    erro_motor: Vec<String>,
    sem_esperado: usize,
    cobertas: BTreeSet<String>,
}

fn valor_txt(v: &Value) -> String {
    match v {
        Value::Empty => "(vazio)".into(),
        Value::Number(n) => n.to_string(),
        Value::Text(s) => format!("\"{s}\""),
        Value::Bool(x) => x.to_string(),
        Value::Error(e) => e.code.as_str().to_string(),
    }
}

fn aba_de_funcoes(c: &mut Ctx, aba: usize, linhas: &[&Linha], rel: &mut Relatorio) {
    escreve_cabecalho(c, aba);
    bloco_de_apoio(c, aba);
    let se = nome_pt("IF");
    let eerro = nome_pt("ISERROR");
    let seerro = nome_pt("IFERROR");
    let enum_ = nome_pt("ISNUMBER");
    let abs = nome_pt("ABS");
    let max = nome_pt("MAX");

    let (dinamicas, normais): (Vec<&Linha>, Vec<&Linha>) = linhas
        .iter()
        .partition(|l| derrama(l));

    let mut r = 1u32;
    for l in &normais {
        let pt = nome_pt(&l.canon);
        let formula = if let Some(inteira) = l.args.strip_prefix('@') {
            format!("={inteira}")
        } else {
            format!("={pt}({})", l.args)
        };
        c.put_rc(aba, r, 0, &pt);
        c.put_rc(aba, r, 1, &l.canon);
        c.put_texto(aba, r, 2, &formula);
        if l.fmt == "so_texto" {
            c.put_rc(aba, r, 3, "(não executada: acessa a rede)");
        } else {
            c.put_rc(aba, r, 3, &formula);
        }
        match l.fmt.as_str() {
            "data" => c.estilo(aba, &format!("D{}", r + 1), &[("numFmt", t("dd/mm/yyyy"))]),
            "datahora" => c.estilo(aba, &format!("D{}", r + 1), &[("numFmt", t("dd/mm/yyyy hh:mm:ss"))]),
            "hora" => c.estilo(aba, &format!("D{}", r + 1), &[("numFmt", t("hh:mm:ss"))]),
            _ => {}
        }
        let n = r + 1;
        let (tipo, resto) = l.esperado.split_once(':').unwrap_or((l.esperado.as_str(), ""));
        match tipo {
            "n" => {
                let v: f64 = resto.parse().unwrap();
                let txt = format!("{v}").replace('.', ",");
                c.put_rc(aba, r, 4, &txt);
                if l.fmt == "data" {
                    c.estilo(aba, &format!("E{n}"), &[("numFmt", t("dd/mm/yyyy"))]);
                } else if l.fmt == "hora" {
                    c.estilo(aba, &format!("E{n}"), &[("numFmt", t("hh:mm:ss"))]);
                }
            }
            "t" => c.put_texto(aba, r, 4, resto),
            "b" => c.put_rc(aba, r, 4, if resto == "1" { "VERDADEIRO" } else { "FALSO" }),
            _ => {}
        }
        if tipo == "err" {
            c.put_rc(aba, r, 4, "#N/D (erro esperado)");
            c.put_rc(aba, r, 5, &format!("={se}({eerro}(D{n});\"ok\";\"DIFERENTE\")"));
        } else if !tipo.is_empty() {
            c.put_rc(
                aba,
                r,
                5,
                &format!(
                    "={se}({eerro}(D{n});\"ERRO\";{seerro}({se}({enum_}(E{n});{se}({abs}(D{n}-E{n})<=0,000001*{max}(1;{abs}(E{n}));\"ok\";\"DIFERENTE\");{se}(D{n}=E{n};\"ok\";\"DIFERENTE\"));\"DIFERENTE\"))"
                ),
            );
        }
        rel.cobertas.insert(l.canon.clone());
        *rel.por_aba.entry(l.aba.clone()).or_default() += 1;
        r += 1;
    }
    // Funções de matriz: 12 linhas livres cada, derramando a partir da coluna D.
    if !dinamicas.is_empty() {
        r += 2;
        c.put_rc(aba, r, 0, "Funções de matriz (derramam: espaço livre abaixo e à direita da coluna D)");
        c.estilo(aba, &format!("A{}", r + 1), &[("bold", b(true))]);
        r += 1;
        for l in &dinamicas {
            let pt = nome_pt(&l.canon);
            let formula = format!("={pt}({})", l.args);
            c.put_rc(aba, r, 0, &pt);
            c.put_rc(aba, r, 1, &l.canon);
            c.put_texto(aba, r, 2, &formula);
            c.put_rc(aba, r, 3, &formula);
            rel.cobertas.insert(l.canon.clone());
            *rel.por_aba.entry(l.aba.clone()).or_default() += 1;
            r += 12;
        }
    }
}

fn confere_resultados(c: &Ctx, aba: usize, linhas: &[&Linha], rel: &mut Relatorio) {
    let (dinamicas, normais): (Vec<&Linha>, Vec<&Linha>) = linhas
        .iter()
        .partition(|l| derrama(l));
    let mut r = 1u32;
    for l in &normais {
        if l.fmt == "so_texto" {
            r += 1;
            continue;
        }
        let v = c.wb.valor(aba, r, 3);
        let conf = c.wb.valor(aba, r, 5);
        let tem_esperado = !l.esperado.is_empty();
        if l.esperado.starts_with("err") {
            // erro esperado
        } else if let Value::Error(e) = &v {
            rel.erro_motor.push(format!("{} / {} -> {}", l.aba, l.canon, e.code.as_str()));
        }
        if tem_esperado {
            match conf {
                Value::Text(ref s) if s == "ok" => rel.ok += 1,
                other => rel.diferente.push(format!(
                    "{} / {} ({}): motor = {}, esperado = {}  [{}]",
                    l.aba,
                    l.canon,
                    l.args,
                    valor_txt(&v),
                    l.esperado,
                    valor_txt(&other)
                )),
            }
        } else {
            rel.sem_esperado += 1;
        }
        r += 1;
    }
    r += 3;
    for l in &dinamicas {
        let v = c.wb.valor(aba, r, 3);
        if let Value::Error(e) = &v {
            rel.erro_motor.push(format!("{} / {} (matriz) -> {}", l.aba, l.canon, e.code.as_str()));
        }
        rel.sem_esperado += 1;
        r += 12;
    }
}

// ------------------------------------------------------------------ gráficos

struct Graf {
    tipo: ChartType,
    titulo: &'static str,
    range: ChartRange,
    cab: bool,
    cat: bool,
    x: f64,
    y: f64,
}

fn aba_graficos(c: &mut Ctx, aba: usize, avisos: &mut Vec<String>) -> usize {
    c.put(aba, "A1", "Gráficos: um de cada tipo do motor (25). Dados nas tabelas abaixo; os gráficos ficam à direita.");
    c.estilo(aba, "A1", &[("bold", b(true))]);
    // T1: categorias x 3 séries (linhas 3..9)
    let t1 = [("Mês", "Vendas", "Meta", "Custo"), ("Jan", "120", "100", "80"), ("Fev", "135", "110", "85"),
        ("Mar", "150", "120", "90"), ("Abr", "110", "130", "95"), ("Mai", "170", "140", "100"), ("Jun", "190", "150", "105")];
    for (i, row) in t1.iter().enumerate() {
        for (j, v) in [row.0, row.1, row.2, row.3].iter().enumerate() {
            c.put_rc(aba, 2 + i as u32, j as u32, v);
        }
    }
    // T2: dispersão X/Y (F3:G10)
    c.put(aba, "F3", "X");
    c.put(aba, "G3", "Y");
    for (i, (x, y)) in [(1, 2), (2, 4), (3, 5), (4, 4), (5, 7), (6, 9), (7, 8)].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 5, &x.to_string());
        c.put_rc(aba, 3 + i as u32, 6, &y.to_string());
    }
    // T3: bolhas X/Y/tamanho (I3:K9)
    for (j, h) in ["X", "Y", "Tamanho"].iter().enumerate() {
        c.put_rc(aba, 2, 8 + j as u32, h);
    }
    for (i, (x, y, z)) in [(1, 5, 10), (2, 8, 20), (3, 4, 15), (4, 9, 30), (5, 6, 25), (6, 7, 12)].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 8, &x.to_string());
        c.put_rc(aba, 3 + i as u32, 9, &y.to_string());
        c.put_rc(aba, 3 + i as u32, 10, &z.to_string());
    }
    // T4: histograma (M3:M22) — uma coluna de números
    c.put(aba, "M3", "Amostra");
    let hist = [2, 3, 3, 4, 4, 4, 5, 5, 5, 5, 6, 6, 6, 7, 7, 8, 9, 10, 12, 15];
    for (i, v) in hist.iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 12, &v.to_string());
    }
    // T5: cascata (O3:P9)
    c.put(aba, "O3", "Etapa");
    c.put(aba, "P3", "Variação");
    for (i, (n, v)) in [("Início", 100), ("Receita", 60), ("Custos", -40), ("Impostos", -15), ("Bônus", 25), ("Fim", 130)].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 14, n);
        c.put_rc(aba, 3 + i as u32, 15, &v.to_string());
    }
    // T6: candlestick (R3:V9) data/abertura/máxima/mínima/fechamento
    for (j, h) in ["Dia", "Abertura", "Máxima", "Mínima", "Fechamento"].iter().enumerate() {
        c.put_rc(aba, 2, 17 + j as u32, h);
    }
    for (i, (d, a, mx, mn, f)) in [("D1", 10, 14, 9, 13), ("D2", 13, 15, 11, 12), ("D3", 12, 16, 12, 15), ("D4", 15, 18, 14, 17), ("D5", 17, 19, 13, 14), ("D6", 14, 17, 13, 16)].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 17, d);
        for (j, v) in [a, mx, mn, f].iter().enumerate() {
            c.put_rc(aba, 3 + i as u32, 18 + j as u32, &v.to_string());
        }
    }
    // T7: Gantt (X3:Z8) tarefa/início/duração
    for (j, h) in ["Tarefa", "Início", "Término"].iter().enumerate() {
        c.put_rc(aba, 2, 23 + j as u32, h);
    }
    for (i, (n, s, d)) in [("Projeto", 0, 4), ("Obra", 3, 9), ("Testes", 8, 11), ("Entrega", 11, 12)].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 23, n);
        c.put_rc(aba, 3 + i as u32, 24, &s.to_string());
        c.put_rc(aba, 3 + i as u32, 25, &d.to_string());
    }
    // T8: mapa de calor (AB3:AF8) matriz categorias x séries
    for (j, h) in ["Região", "Seg", "Ter", "Qua", "Qui"].iter().enumerate() {
        c.put_rc(aba, 2, 27 + j as u32, h);
    }
    for (i, n) in ["Norte", "Sul", "Leste", "Oeste", "Centro"].iter().enumerate() {
        c.put_rc(aba, 3 + i as u32, 27, n);
        for j in 0..4u32 {
            c.put_rc(aba, 3 + i as u32, 28 + j, &(((i as u32 * 7 + j * 13) % 17) + 1).to_string());
        }
    }
    // T9: linha 3D (AH3:AK10) Grupo/X/Y/Z
    for (j, h) in ["Grupo", "X", "Y", "Z"].iter().enumerate() {
        c.put_rc(aba, 2, 33 + j as u32, h);
    }
    for i in 0..8u32 {
        c.put_rc(aba, 3 + i, 33, if i < 4 { "G1" } else { "G2" });
        c.put_rc(aba, 3 + i, 34, &(i % 4 + 1).to_string());
        c.put_rc(aba, 3 + i, 35, &((i * 3) % 7 + 1).to_string());
        c.put_rc(aba, 3 + i, 36, &((i * 5) % 6 + 2).to_string());
    }
    let r = |r0, c0, r1, c1| ChartRange { r0, c0, r1, c1 };
    let t1r = r(2, 0, 8, 3);
    let mut graficos = vec![
        Graf { tipo: ChartType::Column, titulo: "column - Colunas", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::ColumnStacked, titulo: "column-stacked - Colunas empilhadas", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::ColumnStacked100, titulo: "column-stacked-100 - Colunas 100%", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Bar, titulo: "bar - Barras", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::BarStacked, titulo: "bar-stacked - Barras empilhadas", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::BarStacked100, titulo: "bar-stacked-100 - Barras 100%", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Line, titulo: "line - Linhas", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepLine, titulo: "step-line - Linhas em degraus", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepLineStacked, titulo: "step-line-stacked - Degraus empilhados", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepLineStacked100, titulo: "step-line-stacked-100 - Degraus 100%", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Area, titulo: "area - Área", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepArea, titulo: "step-area - Área em degraus", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepAreaStacked, titulo: "step-area-stacked - Área em degraus empilhada", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::StepAreaStacked100, titulo: "step-area-stacked-100 - Área em degraus 100%", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Pie, titulo: "pie - Pizza", range: r(2, 0, 8, 1), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Radar, titulo: "radar - Radar", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Combo, titulo: "combo - Combinado", range: t1r, cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Scatter, titulo: "scatter - Dispersão", range: r(2, 5, 9, 6), cab: true, cat: false, x: 0., y: 0. },
        Graf { tipo: ChartType::Bubble, titulo: "bubble - Bolhas", range: r(2, 8, 8, 10), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Histogram, titulo: "histogram - Histograma", range: r(2, 12, 22, 12), cab: true, cat: false, x: 0., y: 0. },
        Graf { tipo: ChartType::Waterfall, titulo: "waterfall - Cascata", range: r(2, 14, 8, 15), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Candlestick, titulo: "candlestick - Velas", range: r(2, 17, 8, 21), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Gantt, titulo: "gantt - Gantt", range: r(2, 23, 6, 25), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Heatmap, titulo: "heatmap - Mapa de calor", range: r(2, 27, 7, 31), cab: true, cat: true, x: 0., y: 0. },
        Graf { tipo: ChartType::Line3d, titulo: "line-3d - Linha 3D", range: r(2, 34, 10, 36), cab: true, cat: false, x: 0., y: 0. },
    ];
    for (i, g) in graficos.iter_mut().enumerate() {
        g.x = 20.0 + (i % 3) as f64 * 500.0;
        g.y = 330.0 + (i / 3) as f64 * 330.0;
    }
    let mut lista = Vec::new();
    let nome_aba = "Gráficos".to_string();
    for (i, g) in graficos.iter().enumerate() {
        let tipo_nome = serde_json::to_value(g.tipo).unwrap();
        let cfg = ChartConfig {
            tipo: g.tipo,
            title: g.titulo.into(),
            range: g.range,
            series_in_columns: true,
            has_headers: g.cab,
            has_category_labels: g.cat,
            ..Default::default()
        };
        // Confere que o motor consegue extrair e desenhar este tipo com estes dados.
        let sheet = &c.wb.aba(aba).unwrap().sheet;
        let dados = extract_chart_data(sheet, &cfg);
        match renderizar_dados(&cfg, &dados, 800., 450.) {
            Ok(svg) if svg.contains("<svg") => {}
            Ok(_) => avisos.push(format!("{}: SVG vazio", g.titulo)),
            Err(e) => avisos.push(format!("{}: {e}", g.titulo)),
        }
        if dados.series.is_empty() {
            avisos.push(format!("{}: nenhuma série extraída", g.titulo));
        }
        lista.push(json!({
            "id": format!("amostra-{i:02}"),
            "type": tipo_nome,
            "title": g.titulo,
            "range": {"r0": g.range.r0, "c0": g.range.c0, "r1": g.range.r1, "c1": g.range.c1},
            "seriesInColumns": true,
            "hasHeaders": g.cab,
            "hasCategoryLabels": g.cat,
            "aggregate": false,
            "aggregateFn": "sum",
            "style": {"showTitle": true, "legendPosition": "bottom"},
            "position": {"x": g.x, "y": g.y, "width": 480.0, "height": 300.0},
        }));
        let _ = &nome_aba;
    }
    let n = lista.len();
    c.wb.editar(aba, |s| {
        s.g = Some(serde_json::to_string(&lista).unwrap());
    });
    n
}

// -------------------------------------------------------------------- extras

fn aba_extras(c: &mut Ctx, aba: usize) {
    c.put(aba, "A1", "Extras para testar a conversão (.xlsx e Google Planilhas)");
    c.estilo(aba, "A1:F1", &[("bold", b(true)), ("fontSize", Campo::Numero(14.0))]);
    c.wb.editar(aba, |s| {
        feathersheet_io::mesclas::mesclar(s, Rect::new(0, 0, 0, 5));
    });
    c.put(aba, "A3", "Formatação de número");
    c.estilo(aba, "A3", &[("bold", b(true))]);
    let fmts = [
        ("Milhar com 2 casas", "1234567,891", "#,##0.00"),
        ("Percentual", "0,256", "0.00%"),
        ("Data longa", "=DATA(2026;3;15)", "dddd, d \"de\" mmmm \"de\" yyyy"),
        ("Sufixo kWp", "5,4", "0.0\" kWp\""),
        ("Científico", "123456", "0.00E+00"),
        ("4 seções com cor (positivo)", "1234,5", "[Blue]#,##0.00;[Red]-#,##0.00;[Green]\"zero\";[Magenta]@"),
        ("4 seções com cor (negativo)", "-1234,5", "[Blue]#,##0.00;[Red]-#,##0.00;[Green]\"zero\";[Magenta]@"),
        ("4 seções com cor (zero)", "0", "[Blue]#,##0.00;[Red]-#,##0.00;[Green]\"zero\";[Magenta]@"),
        ("4 seções com cor (texto)", "texto", "[Blue]#,##0.00;[Red]-#,##0.00;[Green]\"zero\";[Magenta]@"),
        ("Moeda", "1999,9", "\"R$\" #,##0.00"),
        ("Fração", "0,75", "# ?/?"),
        ("Hora", "0,5625", "hh:mm"),
    ];
    for (i, (nome, valor, codigo)) in fmts.iter().enumerate() {
        let r = 3 + i as u32;
        c.put_rc(aba, r, 0, nome);
        c.put_rc(aba, r, 1, valor);
        c.put_texto(aba, r, 2, codigo);
        c.estilo(aba, &format!("B{}", r + 1), &[("numFmt", t(codigo))]);
    }
    // Comentário
    c.put(aba, "A17", "Comentário na célula ao lado");
    c.put(aba, "B17", "passe o mouse");
    c.wb.editar(aba, |s| {
        s.definir_comentario(16, 1, Some(Comentario {
            autor: "FeatherSheet".into(),
            texto: "Comentário de teste: acentuação, çã, e quebra\nsegunda linha".into(),
            visivel: false,
        }));
    });
    // Validação de dados
    c.put(aba, "A19", "Validação: lista (Baixa/Média/Alta)");
    c.put(aba, "B19", "Média");
    c.put(aba, "A20", "Validação: inteiro entre 1 e 10");
    c.put(aba, "B20", "5");
    c.put(aba, "A21", "Validação: data após 01/01/2026");
    c.put(aba, "B21", "=DATA(2026;6;1)");
    c.estilo(aba, "B21", &[("numFmt", t("dd/mm/yyyy"))]);
    let va = json!([
        {"ranges": [{"r0":18,"c0":1,"r1":18,"c1":1}], "regra": {"tipo":"lista","valores":["Baixa","Média","Alta"]},
         "quando": "recusar", "mensagem": "Escolha Baixa, Média ou Alta.",
         "mensagens": {"entrada_titulo":"Prioridade","entrada_texto":"Escolha na lista.","erro_titulo":"Valor inválido"}},
        {"ranges": [{"r0":19,"c0":1,"r1":19,"c1":1}], "regra": {"tipo":"numero","op":"entre","a":1.0,"b":10.0,"inteiro":true},
         "quando": "avisar", "mensagem": "Use um inteiro de 1 a 10."},
        {"ranges": [{"r0":20,"c0":1,"r1":20,"c1":1}], "regra": {"tipo":"data","op":"depois","a":"2026-01-01"},
         "quando": "informar", "mensagem": "Data deveria ser depois de 01/01/2026."}
    ]);
    c.wb.editar(aba, |s| s.va = Some(va.to_string()));
    // Formatação condicional
    c.put(aba, "A23", "Condicional: > 10 fica vermelho; escala de cor; barra de dados");
    for i in 0..8u32 {
        c.put_rc(aba, 23 + i, 1, &((i * 3 + 1) % 17).to_string());
        c.put_rc(aba, 23 + i, 2, &((i * 5 + 2) % 19).to_string());
        c.put_rc(aba, 23 + i, 3, &((i * 7 + 3) % 23).to_string());
    }
    let cond = json!([
        {"id":"r1","ranges":[{"r0":23,"c0":1,"r1":30,"c1":1}],
         "condition":{"kind":"cell","op":"greaterThan","values":["10"],"style":{"bg":"#ffc7ce","color":"#9c0006","bold":true}}},
        {"id":"r2","ranges":[{"r0":23,"c0":2,"r1":30,"c1":2}],
         "condition":{"kind":"colorScale","min":{"type":"min","color":"#f8696b"},"mid":{"type":"percentile","value":50.0,"color":"#ffeb84"},"max":{"type":"max","color":"#63be7b"}}},
        {"id":"r3","ranges":[{"r0":23,"c0":3,"r1":30,"c1":3}],
         "condition":{"kind":"dataBar","color":"#638ec6"}}
    ]);
    c.wb.editar(aba, |s| s.r = Some(cond.to_string()));
    // Nomes definidos
    c.put(aba, "A33", "Nome definido 'Vendas' = Gráficos!B4:B9");
    c.put(aba, "B33", "=SOMA(Vendas)");
    c.wb.definir_nome("Vendas", Rect { r0: 3, c0: 1, r1: 8, c1: 1, sheet: Some("Gráficos".into()) });
    c.put(aba, "A34", "Nome definido 'Dados' = Matemática!N2:N11");
    c.put(aba, "B34", "=MÉDIA(Dados)");
    c.wb.definir_nome("Dados", Rect { r0: 1, c0: 13, r1: 10, c1: 13, sheet: Some("Matemática".into()) });
    // Mesclagem e recuo
    c.put(aba, "A36", "Mesclagem A36:C36 (centralizada)");
    c.wb.editar(aba, |s| {
        feathersheet_io::mesclas::mesclar(s, Rect::new(35, 0, 35, 2));
    });
    c.estilo(aba, "A36", &[("align", t("center")), ("bg", t("#ddebf7")), ("bold", b(true))]);
    c.put(aba, "A38", "Recuo 0");
    c.put(aba, "A39", "Recuo 1");
    c.put(aba, "A40", "Recuo 3");
    c.wb.editar(aba, |s| {
        feathersheet_core::operacoes::recuo::alterar_recuo(s, &Rect::cell(38, 0), 1);
        feathersheet_core::operacoes::recuo::alterar_recuo(s, &Rect::cell(39, 0), 3);
    });
    // Bordas, fonte, cor, quebra de texto
    c.put(aba, "A42", "Texto longo com quebra de linha automática para testar altura de linha e alinhamento vertical");
    c.estilo(aba, "A42", &[("wrap", b(true)), ("italic", b(true)), ("color", t("#c00000")), ("fontFamily", t("Georgia"))]);
    c.largura(aba, 0, 300);
    c.largura(aba, 1, 150);
    c.largura(aba, 2, 330);
}

fn civil_hoje() -> String {
    let dias = (std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0)
        / 86_400) as i64;
    let z = dias + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
    let y = yoe + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = doy - (153 * mp + 2) / 5 + 1;
    let m = if mp < 10 { mp + 3 } else { mp - 9 };
    let y = if m <= 2 { y + 1 } else { y };
    format!("{d:02}/{m:02}/{y}")
}

fn aba_indice(c: &mut Ctx, aba: usize, rel: &Relatorio, n_graficos: usize, registro: usize, nomes_total: usize) {
    c.put(aba, "A1", "FeatherSheet — Planilha de amostra de ouro");
    c.estilo(aba, "A1", &[("bold", b(true)), ("fontSize", Campo::Numero(16.0))]);
    c.put(aba, "A2", &format!("Gerada em {} pelo exemplo amostra_ouro (feathersheet-io).", civil_hoje()));
    c.put(
        aba,
        "A3",
        &format!(
            "Funções no registro do motor: {registro} (nomes canônicos); {nomes_total} nomes digitáveis, contando apelidos em português. Gráficos: {n_graficos}."
        ),
    );
    c.put(aba, "A5", "Aba");
    c.put(aba, "B5", "Funções");
    c.put(aba, "C5", "O que tem");
    c.estilo(aba, "A5:C5", &[("bold", b(true)), ("bg", t("#1f4e79")), ("color", t("#ffffff"))]);
    let mut r = 5u32;
    let descr = |n: &str| match n {
        "Compatibilidade" => "nomes antigos do Excel (BINOMDIST, NORMSINV…)",
        "Fonte de dados" => "importam da rede: só a fórmula em texto, não executada",
        "Matriz dinâmica" => "derramam: veja a seção ao fim da aba",
        _ => "fórmula viva, esperado e conferência",
    };
    for nome in ABAS {
        let n = rel.por_aba.get(nome).copied().unwrap_or(0);
        c.put_rc(aba, r, 0, &format!("=HIPERLINK(\"#'{nome}'!A1\";\"{nome}\")"));
        c.put_rc(aba, r, 1, &n.to_string());
        c.put_rc(aba, r, 2, descr(nome));
        r += 1;
    }
    for (nome, d) in [("Gráficos", "um gráfico de cada tipo (25); .fshz apenas — o .xlsx do projeto ainda não grava gráficos"),
        ("Extras", "formatos de número, comentário, validação, condicional, nomes, mesclagem, recuo")] {
        c.put_rc(aba, r, 0, &format!("=HIPERLINK(\"#'{nome}'!A1\";\"{nome}\")"));
        let qtd = if nome == "Gráficos" { n_graficos.to_string() } else { "-".to_string() };
        c.put_rc(aba, r, 1, &qtd);
        c.put_rc(aba, r, 2, d);
        r += 1;
    }
    c.put_rc(aba, r, 0, "Total de funções");
    c.put_rc(aba, r, 1, &format!("=SOMA(B6:B{})", 5 + ABAS.len()));
    c.estilo(aba, &format!("A{}:B{}", r + 1, r + 1), &[("bold", b(true))]);
    r += 2;
    c.put_rc(aba, r, 0, "Legenda das colunas (abas de função)");
    c.estilo(aba, &format!("A{}", r + 1), &[("bold", b(true))]);
    for (i, (k, v)) in [
        ("A  Função (pt)", "nome em português, como se digita"),
        ("B  Nome inglês", "nome canônico do motor / Excel"),
        ("C  Fórmula (texto)", "a fórmula como texto, para ler"),
        ("D  Resultado", "a fórmula viva (argumentos na área de dados de apoio, coluna M em diante)"),
        ("E  Esperado (Excel)", "valor que o Excel daria; vazio quando volátil ou desconhecido"),
        ("F  Confere", "ok / DIFERENTE / ERRO; vazio sem esperado"),
    ]
    .iter()
    .enumerate()
    {
        c.put_rc(aba, r + 1 + i as u32, 0, k);
        c.put_rc(aba, r + 1 + i as u32, 2, v);
    }
    c.put_rc(aba, r + 8, 0, "Voláteis (AGORA, HOJE, ALEATÓRIO…) e funções de matriz não têm \"Esperado\".");
    c.largura(aba, 0, 220);
    c.largura(aba, 1, 80);
    c.largura(aba, 2, 520);
}

/// Origem da dinâmica: vendas por região e produto.
const VENDAS: [[&str; 4]; 9] = [
    ["Região", "Produto", "Ano", "Valor"],
    ["Sul", "A", "2024", "10"],
    ["Sul", "B", "2024", "20"],
    ["Norte", "A", "2025", "30"],
    ["Norte", "B", "2025", "5"],
    ["Leste", "A", "2024", "7"],
    ["Norte", "C", "2025", "1"],
    ["Leste", "C", "2025", "12"],
    ["Sul", "C", "2025", "8"],
];

/// Tabela dinâmica (Região × soma de Valor), no formato que o painel grava
/// em `sheet.dn`; o resultado já vai nas células e o Excel recalcula ao abrir.
fn aba_dinamica(c: &mut Ctx, dados: usize, resumo: usize) {
    for (r, linha) in VENDAS.iter().enumerate() {
        for (col, t) in linha.iter().enumerate() {
            c.put_rc(dados, r as u32, col as u32, t);
        }
    }
    let resultado = [["Rótulos de Linha", "Soma de Valor"], ["Leste", "19"], ["Norte", "36"], ["Sul", "38"], ["Total Geral", "93"]];
    for (r, linha) in resultado.iter().enumerate() {
        for (col, t) in linha.iter().enumerate() {
            c.put_rc(resumo, r as u32, col as u32, t);
        }
    }
    let nome_dados = c.wb.aba(dados).unwrap().sheet.name.clone();
    let meta = json!([{
        "id": "dinamica_nativa_1_1",
        "fonte": {"aba": nome_dados, "area": {"r0": 0, "r1": VENDAS.len() - 1, "c0": 0, "c1": 3}},
        "destino": {"row": 0, "col": 0},
        "linhas": [0], "colunas": [], "valores": [{"col": 3, "como": "soma"}], "datas": [],
        "areaResultado": {"r0": 0, "c0": 0, "r1": resultado.len() - 1, "c1": 1},
        "layout": "compacto", "repetirRotulos": false, "subtotais": false,
        "tema": "padrao", "recolhidos": [], "filtros": [], "calculados": [],
    }]);
    c.wb.aba_mut(resumo).unwrap().sheet.dn = Some(meta.to_string());
}

/// Referência a outro arquivo: grava `Origem.fshz` ao lado e cita a aba
/// "Dados" dele, com os valores já guardados (vão para o `externalLink`).
fn aba_vinculos(c: &mut Ctx, aba: usize, saida: &std::path::Path) -> Result<(), String> {
    let mut origem = Workbook::new();
    origem.renomear_aba(0, "Dados");
    for (r, (mes, v)) in [("Mês", "Total"), ("Janeiro", "1200"), ("Fevereiro", "980"), ("Março", "1430")].iter().enumerate() {
        origem.escrever_celula(0, r as u32, 0, mes.to_string());
        origem.escrever_celula(0, r as u32, 1, v.to_string());
    }
    let caminho = saida.join("Origem.fshz");
    feathersheet_io::da_workbook::pasta_gravar_arquivo(&origem, 0, &caminho)?;
    c.put(aba, "A1", "Vínculo com outro arquivo (Origem.fshz, na mesma pasta)");
    c.put(aba, "A3", "Janeiro");
    c.put(aba, "B3", "=[Origem.fshz]Dados!B2");
    c.put(aba, "A4", "Soma do trimestre");
    c.put(aba, "B4", "=SOMA([Origem.fshz]Dados!B2:B4)");
    c.put(aba, "A5", "Março + 10%");
    c.put(aba, "B5", "=[Origem.fshz]Dados!B4*1,1");
    let abas = feathersheet_core::planilha::vinculo::retrato_de_valores(&origem);
    c.wb.instalar_externo_com("Origem.fshz", abas, Some(caminho.display().to_string()), None);
    Ok(())
}

fn main() -> Result<(), String> {
    set_locale_config(LocaleConfig::pt_br());
    let saida: PathBuf = std::env::args()
        .nth(1)
        .map(PathBuf::from)
        .unwrap_or_else(|| std::path::PathBuf::from(std::env::var("HOME").unwrap_or_default()).join("Documentos/FeatherSheet"));
    std::fs::create_dir_all(&saida).map_err(|e| e.to_string())?;

    // 0) o registro manda: toda função canônica precisa estar na tabela.
    let nomes = reg::function_names();
    let mut registro: BTreeSet<String> = BTreeSet::new();
    for n in &nomes {
        registro.insert(reg::canonical(n).unwrap_or(n).to_string());
    }
    let _ = CATEGORIES; // categorias do motor: informativas (a tabela usa as abas da amostra)
    let linhas = ler_tsv();
    let na_tabela: BTreeSet<String> = linhas.iter().map(|l| l.canon.clone()).collect();
    let faltam: Vec<&String> = registro.difference(&na_tabela).collect();
    let sobram: Vec<&String> = na_tabela.difference(&registro).collect();
    if !faltam.is_empty() || !sobram.is_empty() {
        eprintln!("FALTAM no TSV (estão no registro): {faltam:?}");
        eprintln!("SOBRAM no TSV (não estão no registro): {sobram:?}");
    }

    // 1) monta a pasta
    let mut c = Ctx { wb: Workbook::new() };
    c.wb.renomear_aba(0, "Índice");
    let mut indices = BTreeMap::new();
    for nome in ABAS {
        let i = c.wb.criar_aba(nome);
        indices.insert(nome, i);
    }
    let i_graf = c.wb.criar_aba("Gráficos");
    let i_extras = c.wb.criar_aba("Extras");
    let i_vendas = c.wb.criar_aba("Vendas");
    let i_resumo = c.wb.criar_aba("Dinâmica");
    let i_vinculos = c.wb.criar_aba("Vínculos");

    let mut rel = Relatorio::default();
    for nome in ABAS {
        let doaba: Vec<&Linha> = linhas.iter().filter(|l| l.aba == nome).collect();
        aba_de_funcoes(&mut c, indices[nome], &doaba, &mut rel);
    }
    let mut avisos_graf = Vec::new();
    let n_graficos = aba_graficos(&mut c, i_graf, &mut avisos_graf);
    aba_extras(&mut c, i_extras);
    aba_dinamica(&mut c, i_vendas, i_resumo);
    aba_vinculos(&mut c, i_vinculos, &saida)?;
    aba_indice(&mut c, 0, &rel, n_graficos, registro.len(), nomes.len());

    // 2) confere resultados do motor
    c.wb.calcular_em_paralelo();
    for nome in ABAS {
        let doaba: Vec<&Linha> = linhas.iter().filter(|l| l.aba == nome).collect();
        confere_resultados(&c, indices[nome], &doaba, &mut rel);
    }

    // 3) grava .fshz e .xlsx
    let fshz = saida.join("amostra-ouro.fshz");
    let xlsx = saida.join("amostra-ouro.xlsx");
    feathersheet_io::da_workbook::pasta_gravar_arquivo(&c.wb, 0, &fshz)?;
    let bytes = feathersheet_io::xlsx_write::gerar_xlsx(&c.wb)?;
    std::fs::write(&xlsx, &bytes).map_err(|e| e.to_string())?;

    // 4) reabre o .xlsx com o leitor do projeto e compara as fórmulas
    let (volta, _) = feathersheet_io::xlsx_read::ler_xlsx_workbook(bytes)?;
    let mut iguais = 0usize;
    let mut so_nome = 0usize;
    let mut mudaram: Vec<String> = Vec::new();
    let mut valor_difere: Vec<String> = Vec::new();
    let mut total_formulas = 0usize;
    for i in 0..c.wb.len() {
        let nome = c.wb.aba(i).unwrap().sheet.name.clone();
        let Some(j) = volta.indice_por_nome(&nome) else {
            mudaram.push(format!("aba '{nome}' sumiu na volta"));
            continue;
        };
        let ur = c.wb.aba(i).unwrap().sheet.used_rect();
        for r in ur.r0..=ur.r1 {
            for col in ur.c0..=ur.c1 {
                let cru = c.wb.aba(i).unwrap().sheet.get_raw(r, col);
                if !cru.starts_with('=') {
                    continue;
                }
                total_formulas += 1;
                let cru2 = volta.aba(j).unwrap().sheet.get_raw(r, col);
                let pos = format!("{nome}!{}{}", col_name(col), r + 1);
                if cru == cru2 {
                    iguais += 1;
                } else if feathersheet_io::xlsx_write::formula_to_excel(&cru)
                    == feathersheet_io::xlsx_write::formula_to_excel(&cru2)
                {
                    so_nome += 1;
                } else {
                    mudaram.push(format!("{pos}: {cru}   ==>   {cru2}"));
                }
                let v1 = c.wb.valor(i, r, col);
                let v2 = volta.valor(j, r, col);
                let parecido = match (&v1, &v2) {
                    (Value::Number(a), Value::Number(b)) => (a - b).abs() <= 1e-9 * a.abs().max(1.0),
                    (a, b) => a == b,
                };
                // voláteis mudam de propósito
                let volatil = cru.to_uppercase().contains("ALEAT") || cru.contains("AGORA") || cru.contains("HOJE");
                if !parecido && !volatil {
                    valor_difere.push(format!("{pos}: {} ==> {}   [{cru}]", valor_txt(&v1), valor_txt(&v2)));
                }
            }
        }
    }
    // gráficos e extras na volta
    let graf_volta = volta
        .indice_por_nome("Gráficos")
        .and_then(|j| volta.aba(j))
        .map(|a| a.sheet.g.is_some())
        .unwrap_or(false);

    // 4b) reabre também o .fshz gravado em disco
    let lida = feathersheet_io::fshz_read::ler_fshz(&std::fs::read(&fshz).map_err(|e| e.to_string())?)?;
    let do_fshz = feathersheet_io::para_workbook::para_workbook(&lida);
    let (mut fshz_iguais, mut fshz_total) = (0usize, 0usize);
    for i in 0..c.wb.len() {
        let ur = c.wb.aba(i).unwrap().sheet.used_rect();
        for r in ur.r0..=ur.r1 {
            for col in ur.c0..=ur.c1 {
                let cru = c.wb.aba(i).unwrap().sheet.get_raw(r, col);
                if cru.starts_with('=') {
                    fshz_total += 1;
                    if do_fshz.aba(i).map(|a| a.sheet.get_raw(r, col)).as_deref() == Some(cru.as_str()) {
                        fshz_iguais += 1;
                    }
                }
            }
        }
    }
    let graf_fshz = do_fshz
        .indice_por_nome("Gráficos")
        .and_then(|j| do_fshz.aba(j))
        .and_then(|a| a.sheet.g.as_deref().map(|g| g.matches("\"id\"").count()))
        .unwrap_or(0);

    // 5) relatório
    let mut out = String::new();
    let _ = writeln!(out, "== Arquivos ==\n{}\n{}", fshz.display(), xlsx.display());
    let _ = writeln!(out, "\n== Cobertura do registro ==");
    let _ = writeln!(out, "registro: {} funções canônicas ({} nomes digitáveis)", registro.len(), nomes.len());
    let _ = writeln!(out, "cobertas na amostra: {} | faltam: {faltam:?} | sobram: {sobram:?}", rel.cobertas.len());
    for nome in ABAS {
        let _ = writeln!(out, "  {nome}: {}", rel.por_aba.get(nome).copied().unwrap_or(0));
    }
    let _ = writeln!(out, "\n== Conferência com o Excel (coluna Confere) ==");
    let _ = writeln!(out, "ok: {} | diferente/erro: {} | sem esperado: {}", rel.ok, rel.diferente.len(), rel.sem_esperado);
    for d in &rel.diferente {
        let _ = writeln!(out, "  DIFERENTE {d}");
    }
    let _ = writeln!(out, "\n== Funções com erro no motor ({}) ==", rel.erro_motor.len());
    for d in &rel.erro_motor {
        let _ = writeln!(out, "  {d}");
    }
    let _ = writeln!(out, "\n== Gráficos ==\n{n_graficos} configurados; avisos: {}", avisos_graf.len());
    for a in &avisos_graf {
        let _ = writeln!(out, "  {a}");
    }
    let _ = writeln!(out, "gráficos presentes no .xlsx reaberto: {graf_volta}");
    let _ = writeln!(out, "\n== Reabertura do .fshz ==\nfórmulas idênticas: {fshz_iguais}/{fshz_total} | gráficos na aba Gráficos: {graf_fshz}");
    let _ = writeln!(out, "\n== Reabertura do .xlsx ==");
    let _ = writeln!(out, "fórmulas: {total_formulas} | idênticas: {iguais} | equivalentes (só o nome da função/pontuação mudou, ex. XTIR→XIRR): {so_nome} | diferentes de verdade: {}", mudaram.len());
    for m in &mudaram {
        let _ = writeln!(out, "  MUDOU {m}");
    }
    let _ = writeln!(out, "valores que mudaram na volta (excluindo voláteis): {}", valor_difere.len());
    for m in &valor_difere {
        let _ = writeln!(out, "  VALOR {m}");
    }
    print!("{out}");
    Ok(())
}
