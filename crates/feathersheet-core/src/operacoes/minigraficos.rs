//! Minigráficos (Inserir ▸ Minigráficos): um gráfico pequeno de linha, coluna
//! ou ganhos/perdas desenhado dentro de uma célula, a partir de uma faixa de
//! dados. Como no Excel, ficam em grupos: o grupo guarda o tipo e as cores, e
//! cada item é uma célula com a sua faixa. A aba guarda a lista de grupos em
//! JSON (`Sheet::mg`; ler e gravar o JSON fica no `feathersheet-io`, porque o
//! motor não lê JSON); aqui ficam o modelo e as contas, sem tela.
use serde::{Deserialize, Serialize};

use crate::celulas::refs::{format_ref, parse_range_ref, quote_sheet, Rect};
use crate::operacoes::reflow::ajustar_intervalo_de_regra;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum Tipo {
    #[default]
    #[serde(rename = "linha")]
    Linha,
    #[serde(rename = "coluna")]
    Coluna,
    #[serde(rename = "ganhos")]
    GanhosPerdas,
}

/// Uma célula com minigráfico. `dados` é a referência como o usuário a vê
/// ("B2:M2" ou "'Vendas 2026'!B2:M2"), igual ao que o `.xlsx` guarda.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Minigrafico {
    pub linha: u32,
    pub coluna: u32,
    pub dados: String,
}

fn cor_serie() -> String { "#376092".into() }
fn cor_destaque() -> String { "#D00000".into() }

/// As cores padrão são as do Excel ao inserir um minigráfico.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Grupo {
    #[serde(default)]
    pub tipo: Tipo,
    pub itens: Vec<Minigrafico>,
    #[serde(default = "cor_serie")]
    pub cor: String,
    #[serde(default = "cor_destaque")]
    pub cor_negativo: String,
    #[serde(default = "cor_destaque")]
    pub cor_marcadores: String,
    #[serde(default = "cor_destaque")]
    pub cor_alto: String,
    #[serde(default = "cor_destaque")]
    pub cor_baixo: String,
    #[serde(default = "cor_destaque")]
    pub cor_primeiro: String,
    #[serde(default = "cor_destaque")]
    pub cor_ultimo: String,
    /// Marcadores em todos os pontos (só no tipo linha).
    #[serde(default)]
    pub marcadores: bool,
    #[serde(default)]
    pub alto: bool,
    #[serde(default)]
    pub baixo: bool,
    #[serde(default)]
    pub primeiro: bool,
    #[serde(default)]
    pub ultimo: bool,
    #[serde(default)]
    pub negativos: bool,
    /// Mesmo mínimo e máximo para todos os itens do grupo (no Excel, "Mesmo
    /// para Todos os Minigráficos"); sem isso, cada um usa a própria faixa.
    #[serde(default)]
    pub eixo_igual: bool,
}

impl Grupo {
    pub fn novo(tipo: Tipo, itens: Vec<Minigrafico>) -> Self {
        Grupo {
            tipo, itens,
            cor: cor_serie(), cor_negativo: cor_destaque(), cor_marcadores: cor_destaque(),
            cor_alto: cor_destaque(), cor_baixo: cor_destaque(), cor_primeiro: cor_destaque(),
            cor_ultimo: cor_destaque(),
            marcadores: false, alto: false, baixo: false, primeiro: false, ultimo: false,
            negativos: tipo == Tipo::GanhosPerdas, eixo_igual: false,
        }
    }
}

/// O grupo e o item que estão na célula.
pub fn na_celula(grupos: &[Grupo], linha: u32, coluna: u32) -> Option<(usize, usize)> {
    grupos.iter().enumerate().find_map(|(g, grupo)| {
        grupo.itens.iter().position(|m| m.linha == linha && m.coluna == coluna).map(|i| (g, i))
    })
}

/// A referência de dados separada em (aba, retângulo). Sem aba = a própria.
pub fn partir_dados(dados: &str) -> Option<(Option<String>, Rect)> {
    match dados.rsplit_once('!') {
        Some((aba, faixa)) => {
            let aba = aba.strip_prefix('\'').and_then(|a| a.strip_suffix('\'')).map(|a| a.replace("''", "'"))
                .unwrap_or_else(|| aba.to_owned());
            Some((Some(aba), parse_range_ref(faixa)?))
        }
        None => Some((None, parse_range_ref(dados)?)),
    }
}

fn escrever_dados(aba: Option<&str>, r: &Rect) -> String {
    match aba {
        Some(aba) => format!("{}!{}", quote_sheet(aba), format_ref(r)),
        None => format_ref(r),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ErroCriar {
    /// O local tem mais de uma linha e mais de uma coluna.
    LocalNaoEUmaLinhaOuColuna,
    /// Os dados não se dividem em uma série por célula do local.
    TamanhosNaoBatem,
}

/// Monta um grupo como o diálogo "Criar Minigráficos" do Excel: o local é
/// uma linha ou uma coluna de N células, e os dados se dividem em N séries
/// (uma por linha dos dados, ou uma por coluna). `aba_dados` é o nome da aba
/// dos dados quando ela não é a do local.
pub fn criar(tipo: Tipo, dados: Rect, aba_dados: Option<&str>, local: Rect) -> Result<Grupo, ErroCriar> {
    let (dados, local) = (dados.normalized(), local.normalized());
    let (alt, larg) = (dados.r1 - dados.r0 + 1, dados.c1 - dados.c0 + 1);
    let celulas: Vec<(u32, u32)> = if local.r0 == local.r1 {
        (local.c0..=local.c1).map(|c| (local.r0, c)).collect()
    } else if local.c0 == local.c1 {
        (local.r0..=local.r1).map(|r| (r, local.c0)).collect()
    } else {
        return Err(ErroCriar::LocalNaoEUmaLinhaOuColuna);
    };
    let n = celulas.len() as u32;
    let series: Vec<Rect> = if n == 1 && (alt == 1 || larg == 1) {
        vec![dados.clone()]
    } else if alt == n {
        (dados.r0..=dados.r1).map(|r| Rect::new(r, dados.c0, r, dados.c1)).collect()
    } else if larg == n {
        (dados.c0..=dados.c1).map(|c| Rect::new(dados.r0, c, dados.r1, c)).collect()
    } else {
        return Err(ErroCriar::TamanhosNaoBatem);
    };
    let itens = celulas.into_iter().zip(series)
        .map(|((linha, coluna), s)| Minigrafico { linha, coluna, dados: escrever_dados(aba_dados, &s) })
        .collect();
    Ok(Grupo::novo(tipo, itens))
}

/// Inserir/excluir linhas (`coluna == false`) ou colunas na aba `aba`: as
/// células dos minigráficos e as faixas de dados da própria aba andam junto;
/// item cuja célula ou faixa sumiu é apagado, e grupo vazio também.
/// shortcut: faixa de dados em OUTRA aba não acompanha inserir/excluir feito lá; ligar em `Workbook::mover_estrutura` se aparecer o caso.
pub fn refluir(grupos: &mut Vec<Grupo>, coluna: bool, at: u32, quantas: u32, removendo: bool) {
    let (at, q) = (at as i64, quantas as i64);
    let mover = |i: u32| ajustar_intervalo_de_regra(i as i64, i as i64, at, q, removendo).map(|(a, _)| a as u32);
    for grupo in grupos.iter_mut() {
        grupo.itens.retain_mut(|m| {
            let pos = if coluna { &mut m.coluna } else { &mut m.linha };
            let Some(novo) = mover(*pos) else { return false };
            *pos = novo;
            match partir_dados(&m.dados) {
                Some((None, r)) => {
                    let (a, b) = if coluna { (r.c0, r.c1) } else { (r.r0, r.r1) };
                    let Some((a, b)) = ajustar_intervalo_de_regra(a as i64, b as i64, at, q, removendo) else { return false };
                    let r = if coluna { Rect::new(r.r0, a as u32, r.r1, b as u32) } else { Rect::new(a as u32, r.c0, b as u32, r.c1) };
                    m.dados = format_ref(&r);
                    true
                }
                _ => true,
            }
        });
    }
    grupos.retain(|g| !g.itens.is_empty());
}

/// O que desenhar, em frações do retângulo da célula (0..1, y para baixo).
#[derive(Debug, Clone, PartialEq)]
pub enum Marca {
    /// Linha ligando os pontos; `None` é célula vazia (a linha quebra).
    Linha(Vec<Option<(f64, f64)>>),
    /// Uma barra: x inicial, largura, y de cima, y de baixo e qual cor.
    Barra { x: f64, largura: f64, topo: f64, base: f64, cor: Cor },
    /// Um ponto destacado na linha.
    Ponto { x: f64, y: f64, cor: Cor },
    /// Eixo horizontal no zero (quando a faixa passa pelo zero).
    Eixo(f64),
}

/// Qual cor do grupo usar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Cor { Serie, Negativo, Marcadores, Alto, Baixo, Primeiro, Ultimo }

/// Mínimo e máximo dos números de uma série (ignora vazios).
pub fn limites(valores: &[Option<f64>]) -> Option<(f64, f64)> {
    valores.iter().flatten().fold(None, |acc, &v| match acc {
        None => Some((v, v)),
        Some((a, b)) => Some((a.min(v), b.max(v))),
    })
}

/// O desenho de uma série. `escala` é o (mín, máx) do eixo: a própria série
/// ou o grupo todo (`Grupo::eixo_igual`).
pub fn desenhar(grupo: &Grupo, valores: &[Option<f64>], escala: Option<(f64, f64)>) -> Vec<Marca> {
    let n = valores.len();
    let Some((mut min, mut max)) = escala.or_else(|| limites(valores)) else { return Vec::new() };
    if n == 0 { return Vec::new(); }
    let mut marcas = Vec::new();
    // Destaque de cada índice, na ordem de prioridade do Excel (o último vence).
    let numeros: Vec<(usize, f64)> = valores.iter().enumerate().filter_map(|(i, v)| v.map(|v| (i, v))).collect();
    let alto = numeros.iter().max_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
    let baixo = numeros.iter().min_by(|a, b| a.1.total_cmp(&b.1)).map(|x| x.0);
    let (primeiro, ultimo) = (numeros.first().map(|x| x.0), numeros.last().map(|x| x.0));
    let destaque = |i: usize, v: f64| -> Option<Cor> {
        let mut cor = None;
        if grupo.marcadores && grupo.tipo == Tipo::Linha { cor = Some(Cor::Marcadores); }
        if grupo.negativos && v < 0.0 { cor = Some(Cor::Negativo); }
        if grupo.primeiro && primeiro == Some(i) { cor = Some(Cor::Primeiro); }
        if grupo.ultimo && ultimo == Some(i) { cor = Some(Cor::Ultimo); }
        if grupo.baixo && baixo == Some(i) { cor = Some(Cor::Baixo); }
        if grupo.alto && alto == Some(i) { cor = Some(Cor::Alto); }
        cor
    };
    let passo = 1.0 / n as f64;
    match grupo.tipo {
        Tipo::Linha => {
            if max == min { max += 0.5; min -= 0.5; }
            let y = |v: f64| 1.0 - (v - min) / (max - min);
            let x = |i: usize| if n == 1 { 0.5 } else { i as f64 / (n - 1) as f64 };
            marcas.push(Marca::Linha(valores.iter().enumerate().map(|(i, v)| v.map(|v| (x(i), y(v)))).collect()));
            for &(i, v) in &numeros {
                if let Some(cor) = destaque(i, v) { marcas.push(Marca::Ponto { x: x(i), y: y(v), cor }); }
            }
        }
        Tipo::Coluna => {
            // As barras saem do zero; com tudo positivo o zero fica no pé.
            let (min, max) = (min.min(0.0), max.max(0.0));
            let total = if max > min { max - min } else { 1.0 };
            let y = |v: f64| 1.0 - (v - min) / total;
            let zero = y(0.0);
            if min < 0.0 && max > 0.0 { marcas.push(Marca::Eixo(zero)); }
            for &(i, v) in &numeros {
                let (a, b) = if v >= 0.0 { (y(v), zero) } else { (zero, y(v)) };
                let cor = destaque(i, v).unwrap_or(Cor::Serie);
                marcas.push(Marca::Barra { x: i as f64 * passo + passo * 0.1, largura: passo * 0.8, topo: a, base: b, cor });
            }
        }
        Tipo::GanhosPerdas => {
            // Só o sinal importa: positivo em cima, negativo embaixo, zero em branco.
            for &(i, v) in &numeros {
                if v == 0.0 { continue; }
                let (topo, base) = if v > 0.0 { (0.05, 0.5) } else { (0.5, 0.95) };
                let cor = destaque(i, v).unwrap_or(Cor::Serie);
                marcas.push(Marca::Barra { x: i as f64 * passo + passo * 0.1, largura: passo * 0.8, topo, base, cor });
            }
        }
    }
    marcas
}

#[cfg(test)]
mod tests {
    use super::*;

    fn r(a: &str) -> Rect { parse_range_ref(a).unwrap() }

    #[test]
    fn criar_divide_os_dados_por_linha_ou_coluna() {
        // Local numa coluna de 3 células, dados 3×12: uma série por linha.
        let g = criar(Tipo::Linha, r("B2:M4"), None, r("N2:N4")).unwrap();
        let itens: Vec<_> = g.itens.iter().map(|m| (m.linha, m.coluna, m.dados.as_str())).collect();
        assert_eq!(itens, [(1, 13, "B2:M2"), (2, 13, "B3:M3"), (3, 13, "B4:M4")]);
        // Local numa linha de 2 células, dados 5×2: uma série por coluna, noutra aba.
        let g = criar(Tipo::Coluna, r("A1:B5"), Some("Dados 2026"), r("A7:B7")).unwrap();
        assert_eq!(g.itens[1].dados, "'Dados 2026'!B1:B5");
        assert_eq!(partir_dados(&g.itens[1].dados), Some((Some("Dados 2026".into()), r("B1:B5"))));
        assert_eq!(criar(Tipo::Linha, r("A1:C3"), None, r("E1:F2")), Err(ErroCriar::LocalNaoEUmaLinhaOuColuna));
        assert_eq!(criar(Tipo::Linha, r("A1:C3"), None, r("E1:E2")), Err(ErroCriar::TamanhosNaoBatem));
        assert_eq!(criar(Tipo::Linha, r("A1:C3"), None, r("E1")), Err(ErroCriar::TamanhosNaoBatem));
    }

    #[test]
    fn refluir_anda_com_as_celulas_e_apaga_o_que_some() {
        let mut gs = vec![criar(Tipo::Linha, r("B2:M4"), None, r("N2:N4")).unwrap()];
        // Inserir 2 colunas antes de C: dados esticam, local anda.
        refluir(&mut gs, true, 2, 2, false);
        assert_eq!((gs[0].itens[0].coluna, gs[0].itens[0].dados.as_str()), (15, "B2:O2"));
        // Excluir a linha 3 (índice 2): o item do meio some.
        refluir(&mut gs, false, 2, 1, true);
        let itens: Vec<_> = gs[0].itens.iter().map(|m| (m.linha, m.dados.as_str())).collect();
        assert_eq!(itens, [(1, "B2:O2"), (2, "B3:O3")]);
        // Excluir a coluna do local apaga o grupo inteiro.
        refluir(&mut gs, true, 15, 1, true);
        assert!(gs.is_empty());
    }

    #[test]
    fn desenho_de_cada_tipo() {
        let mut g = Grupo::novo(Tipo::Linha, Vec::new());
        g.alto = true;
        g.baixo = true;
        let v = [Some(1.0), None, Some(3.0), Some(2.0)];
        let m = desenhar(&g, &v, None);
        assert_eq!(m[0], Marca::Linha(vec![Some((0.0, 1.0)), None, Some((2.0 / 3.0, 0.0)), Some((1.0, 0.5))]));
        assert_eq!(&m[1..], &[Marca::Ponto { x: 0.0, y: 1.0, cor: Cor::Baixo }, Marca::Ponto { x: 2.0 / 3.0, y: 0.0, cor: Cor::Alto }]);

        g.tipo = Tipo::Coluna;
        let m = desenhar(&g, &[Some(-1.0), Some(1.0)], None);
        assert_eq!(m[0], Marca::Eixo(0.5));
        assert!(matches!(m[1], Marca::Barra { topo, base, cor: Cor::Baixo, .. } if topo == 0.5 && base == 1.0));
        assert!(matches!(m[2], Marca::Barra { topo, base, cor: Cor::Alto, .. } if topo == 0.0 && base == 0.5));

        let g = Grupo::novo(Tipo::GanhosPerdas, Vec::new());
        let m = desenhar(&g, &[Some(5.0), Some(0.0), Some(-2.0)], None);
        assert_eq!(m.len(), 2, "zero não desenha");
        assert!(matches!(m[1], Marca::Barra { cor: Cor::Negativo, .. }));
        // Escala do grupo: o mesmo valor fica mais baixo com um máximo maior.
        let mut g = Grupo::novo(Tipo::Linha, Vec::new());
        g.eixo_igual = true;
        assert_eq!(desenhar(&g, &[Some(5.0)], Some((0.0, 10.0)))[0], Marca::Linha(vec![Some((0.5, 0.5))]));
        assert!(desenhar(&g, &[None, None], None).is_empty());
    }
}
