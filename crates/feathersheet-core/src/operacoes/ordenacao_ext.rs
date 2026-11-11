//! Classificação além de A→Z: por cor da célula ou da fonte, por lista
//! personalizada (dias da semana, meses…) e da esquerda para a direita
//! (ordenar colunas pelos valores de uma linha).
//!
//! Complementa `ops::ordenacao` (que continua valendo para a classificação por
//! valor, com SortKey). Aqui cada nível é uma [`ChaveOrdem`]; a GUI monta a
//! lista de níveis da janela Classificar e chama
//! [`planejar_por_linhas`] / [`planejar_por_colunas`] e depois aplica.
//!
//! Como no Excel: a cor escolhida vai "em cima" ou "embaixo"; as demais
//! células ficam na ordem em que estavam, a menos que haja outro nível
//! (outra cor, ou valor). Para ordenar por várias cores, um nível por cor.

use crate::celulas::refs::Rect;
use crate::celulas::values::{compare_exato, Value};
use crate::operacoes::filtro::{mesma_cor, AlvoCor, CoresDaCelula};
use crate::planilha::sheet::edicao::Snap;
use crate::planilha::sheet::Sheet;
use std::cmp::Ordering;

pub use crate::operacoes::ops::PlanoDeOrdem;

/// O que um nível de classificação compara.
#[derive(Debug, Clone, PartialEq)]
pub enum CriterioOrdem {
    /// O valor (A→Z / Z→A, menor→maior…).
    Valor,
    /// Células da cor dada primeiro (`em_cima`) ou por último; `None` = sem
    /// preenchimento / fonte automática.
    Cor { alvo: AlvoCor, cor: Option<String>, em_cima: bool },
    /// A ordem de uma lista (`["seg","ter",…]`), sem ligar para maiúsculas.
    /// Fora da lista vai depois, em ordem normal; "decrescente" inverte tudo.
    Lista(Vec<String>),
}

/// Um nível. `indice` é a coluna absoluta (classificar linhas) ou a linha
/// absoluta (da esquerda para a direita). `ascendente` vale para `Valor` e
/// `Lista`; a cor usa `em_cima`.
#[derive(Debug, Clone, PartialEq)]
pub struct ChaveOrdem {
    pub indice: u32,
    pub ascendente: bool,
    pub criterio: CriterioOrdem,
}

/// Listas personalizadas que o Excel traz em pt-BR: (nome, itens).
pub fn listas_prontas() -> Vec<(&'static str, Vec<String>)> {
    // Os itens saem na língua do app (jan → Jan, janv.); o nome da lista é só o rótulo.
    let l = |v: &[&str]| v.iter().map(|s| crate::tr!(s)).collect::<Vec<_>>();
    vec![
        ("dom, seg, ter, qua, qui, sex, sáb", l(&["dom", "seg", "ter", "qua", "qui", "sex", "sáb"])),
        ("domingo, segunda-feira, terça-feira, quarta-feira, quinta-feira, sexta-feira, sábado",
            l(&["domingo", "segunda-feira", "terça-feira", "quarta-feira", "quinta-feira", "sexta-feira", "sábado"])),
        ("jan, fev, mar, abr, mai, jun, jul, ago, set, out, nov, dez",
            l(&["jan", "fev", "mar", "abr", "mai", "jun", "jul", "ago", "set", "out", "nov", "dez"])),
        ("janeiro, fevereiro, março, abril, maio, junho, julho, agosto, setembro, outubro, novembro, dezembro",
            l(&["janeiro", "fevereiro", "março", "abril", "maio", "junho", "julho", "agosto", "setembro", "outubro", "novembro", "dezembro"])),
    ]
}

/// Listas que a pessoa criou (Configurações ▸ Planilha): a alça as continua como as prontas.
/// Global como o idioma: vale para o app inteiro, não para um arquivo.
static LISTAS_DO_USUARIO: std::sync::RwLock<Vec<Vec<String>>> = std::sync::RwLock::new(Vec::new());

/// Troca as listas da pessoa; lista com menos de dois itens não tem o que continuar e sai.
pub fn definir_listas_do_usuario(listas: Vec<Vec<String>>) {
    let listas = listas.into_iter().filter(|l| l.len() >= 2).collect();
    *LISTAS_DO_USUARIO.write().unwrap_or_else(|e| e.into_inner()) = listas;
}

pub fn listas_do_usuario() -> Vec<Vec<String>> {
    LISTAS_DO_USUARIO.read().unwrap_or_else(|e| e.into_inner()).clone()
}

/// Texto de uma lista personalizada digitada (`"baixa; média; alta"` ou uma
/// por linha) em itens.
pub fn lista_de_texto(texto: &str) -> Vec<String> {
    texto
        .split(['\n', ';', ','])
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(str::to_string)
        .collect()
}

fn texto_da_celula(v: &Value) -> String {
    crate::operacoes::filtro::texto_js(v).trim().to_lowercase()
}

/// O que se leu de cada posição, para cada nível.
enum Lido {
    Valores(Vec<Value>),
    /// Posição (no eixo ordenado) bate com a cor? Já resolvido com `em_cima`.
    Cor(Vec<bool>),
    Lista(Vec<Option<usize>>, Vec<Value>),
}

/// Ordena `posicoes` (índices ao longo do eixo: linhas ou colunas). `ler`
/// devolve valor e cores da célula do nível `k` na posição `p`.
fn ordenar_posicoes(
    posicoes: &[u32],
    chaves: &[ChaveOrdem],
    maiusculas: bool,
    ler: &dyn Fn(u32, &ChaveOrdem) -> (Value, CoresDaCelula),
) -> Vec<u32> {
    let lidos: Vec<Lido> = chaves
        .iter()
        .map(|k| {
            let celulas: Vec<(Value, CoresDaCelula)> = posicoes.iter().map(|&p| ler(p, k)).collect();
            match &k.criterio {
                CriterioOrdem::Valor => Lido::Valores(celulas.into_iter().map(|c| c.0).collect()),
                CriterioOrdem::Cor { alvo, cor, .. } => Lido::Cor(
                    celulas
                        .iter()
                        .map(|(_, (fundo, fonte))| {
                            let da_celula = if *alvo == AlvoCor::Fundo { fundo } else { fonte };
                            mesma_cor(da_celula.as_deref(), cor.as_deref())
                        })
                        .collect(),
                ),
                CriterioOrdem::Lista(itens) => {
                    let itens: Vec<String> = itens.iter().map(|s| s.trim().to_lowercase()).collect();
                    let valores: Vec<Value> = celulas.into_iter().map(|c| c.0).collect();
                    let pos = valores
                        .iter()
                        .map(|v| {
                            let t = texto_da_celula(v);
                            itens.iter().position(|i| *i == t)
                        })
                        .collect();
                    Lido::Lista(pos, valores)
                }
            }
        })
        .collect();

    let mut ordem: Vec<usize> = (0..posicoes.len()).collect();
    ordem.sort_by(|&a, &b| {
        for (k, lido) in chaves.iter().zip(&lidos) {
            let o = match lido {
                Lido::Valores(v) => match compare_exato(&v[a], &v[b]) {
                    Ok(mut o) => {
                        if o == Ordering::Equal && maiusculas {
                            o = super::ops::desempate_por_caixa(&v[a], &v[b]);
                        }
                        if k.ascendente { o } else { o.reverse() }
                    }
                    Err(_) => Ordering::Equal,
                },
                Lido::Cor(bate) => {
                    let em_cima = matches!(&k.criterio, CriterioOrdem::Cor { em_cima: true, .. });
                    // `false` < `true`: quem bate vai primeiro quando em cima.
                    if em_cima { bate[b].cmp(&bate[a]) } else { bate[a].cmp(&bate[b]) }
                }
                Lido::Lista(pos, v) => {
                    let o = match (pos[a], pos[b]) {
                        (Some(x), Some(y)) => x.cmp(&y),
                        (Some(_), None) => Ordering::Less,
                        (None, Some(_)) => Ordering::Greater,
                        (None, None) => compare_exato(&v[a], &v[b]).unwrap_or(Ordering::Equal),
                    };
                    if k.ascendente { o } else { o.reverse() }
                }
            };
            if o != Ordering::Equal {
                return o;
            }
        }
        a.cmp(&b)
    });
    ordem.into_iter().map(|i| posicoes[i]).collect()
}

/// Classifica as **linhas** de `area` pelos níveis `chaves` (o `indice` é a
/// coluna). `cores(linha, coluna)` dá preenchimento e fonte. `None` quando
/// nada muda.
pub fn planejar_por_linhas(
    area: Rect,
    chaves: &[ChaveOrdem],
    tem_cabecalho: bool,
    maiusculas: bool,
    max_row: u32,
    valor: &dyn Fn(u32, u32) -> Value,
    cores: &dyn Fn(u32, u32) -> CoresDaCelula,
) -> Option<PlanoDeOrdem> {
    let r = area.normalized();
    let first = r.r0 + u32::from(tem_cabecalho);
    let max_r = r.r1.min(max_row);
    if first >= max_r || chaves.is_empty() {
        return None;
    }
    let posicoes: Vec<u32> = (first..=max_r).collect();
    let ordem = ordenar_posicoes(&posicoes, chaves, maiusculas, &|p, k| (valor(p, k.indice), cores(p, k.indice)));
    if ordem.iter().enumerate().all(|(i, &row)| row == first + i as u32) {
        return None;
    }
    Some(PlanoDeOrdem { first, max_r, ordem })
}

/// O plano de ordenar colunas: `ordem[i]` é a coluna que vai parar em
/// `first + i`; vale para as linhas `r0..=r1`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlanoDeColunas {
    pub first: u32,
    pub last: u32,
    pub r0: u32,
    pub r1: u32,
    pub ordem: Vec<u32>,
}

/// Classifica as **colunas** de `area` (da esquerda para a direita) pelos
/// valores/cores de uma ou mais linhas (o `indice` é a linha). Com
/// `tem_rotulos`, a primeira coluna da área (rótulos das linhas) não anda.
pub fn planejar_por_colunas(
    area: Rect,
    chaves: &[ChaveOrdem],
    tem_rotulos: bool,
    maiusculas: bool,
    max_col: u32,
    valor: &dyn Fn(u32, u32) -> Value,
    cores: &dyn Fn(u32, u32) -> CoresDaCelula,
) -> Option<PlanoDeColunas> {
    let r = area.normalized();
    let first = r.c0 + u32::from(tem_rotulos);
    let last = r.c1.min(max_col);
    if first >= last || chaves.is_empty() {
        return None;
    }
    let posicoes: Vec<u32> = (first..=last).collect();
    let ordem = ordenar_posicoes(&posicoes, chaves, maiusculas, &|p, k| (valor(k.indice, p), cores(k.indice, p)));
    if ordem.iter().enumerate().all(|(i, &c)| c == first + i as u32) {
        return None;
    }
    Some(PlanoDeColunas { first, last, r0: r.r0, r1: r.r1, ordem })
}

/// Move as colunas (conteúdo e estilo; a fórmula anda junto, com as
/// referências relativas deslocadas) num único passo do desfazer.
pub fn aplicar_ordenacao_colunas(sheet: &mut Sheet, plano: &PlanoDeColunas) {
    if plano.ordem.is_empty() {
        return;
    }
    let fotos: Vec<Vec<Option<Snap>>> = plano
        .ordem
        .iter()
        .map(|&c| (plano.r0..=plano.r1).map(|r| sheet.snapshot(r, c)).collect())
        .collect();
    sheet.abrir_transacao();
    for (i, coluna) in fotos.into_iter().enumerate() {
        let destino = plano.first + i as u32;
        let origem = plano.ordem[i];
        for (j, foto) in coluna.into_iter().enumerate() {
            let linha = plano.r0 + j as u32;
            let nova = foto.map(|mut s| {
                if s.raw.starts_with('=') {
                    s.raw = crate::operacoes::fill::shift_formula(&s.raw, 0, destino as i32 - origem as i32);
                }
                s
            });
            sheet.apply(linha, destino, nova, true);
        }
    }
    sheet.fechar_transacao();
}

/// Classifica a aba por linhas, lendo valor e cores direto dela, num passo do
/// desfazer. Devolve se algo mudou.
pub fn ordenar_por_linhas(sheet: &mut Sheet, area: Rect, chaves: &[ChaveOrdem], tem_cabecalho: bool, maiusculas: bool) -> bool {
    let max_row = sheet.max_row_ts();
    let plano = {
        let s = &*sheet;
        planejar_por_linhas(area.clone(), chaves, tem_cabecalho, maiusculas, max_row,
            &|r, c| s.get_value(r, c), &|r, c| crate::operacoes::filtro::cores_da_celula(s, r, c))
    };
    match plano {
        Some(p) => {
            let r = area.normalized();
            super::ops::aplicar_ordenacao(sheet, r.c0, r.c1, &p);
            true
        }
        None => false,
    }
}

/// Classifica a aba da esquerda para a direita (ver [`planejar_por_colunas`]).
pub fn ordenar_por_colunas(sheet: &mut Sheet, area: Rect, chaves: &[ChaveOrdem], tem_rotulos: bool, maiusculas: bool) -> bool {
    let max_col = sheet.max_col_ts();
    let plano = {
        let s = &*sheet;
        planejar_por_colunas(area, chaves, tem_rotulos, maiusculas, max_col,
            &|r, c| s.get_value(r, c), &|r, c| crate::operacoes::filtro::cores_da_celula(s, r, c))
    };
    match plano {
        Some(p) => {
            aplicar_ordenacao_colunas(sheet, &p);
            true
        }
        None => false,
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::estilo_parcial::{Campo, EstiloParcial};

    fn pintar(s: &mut Sheet, r: u32, c: u32, campo: &str, cor: &str) {
        let mut e = EstiloParcial::new();
        e.insert(campo.into(), Campo::Texto(cor.into()));
        s.set_style(&Rect::new(r, c, r, c), &e);
    }

    fn coluna(s: &Sheet, c: u32, n: u32) -> Vec<String> {
        (0..n).map(|r| crate::operacoes::filtro::texto_js(&s.get_value(r, c))).collect()
    }

    fn chave(indice: u32, criterio: CriterioOrdem) -> ChaveOrdem {
        ChaveOrdem { indice, ascendente: true, criterio }
    }

    fn aba_coloria() -> Sheet {
        let mut s = Sheet::new("T");
        for (r, (v, bg, fonte)) in [("c", "#ff0000", ""), ("a", "", "#0000ff"), ("d", "#00ff00", ""), ("b", "#ff0000", "#0000ff"), ("e", "", "")].iter().enumerate() {
            let r = r as u32;
            s.set_cell(r, 0, v);
            s.set_cell(r, 1, &format!("{}", r + 1));
            if !bg.is_empty() { pintar(&mut s, r, 0, "bg", bg); }
            if !fonte.is_empty() { pintar(&mut s, r, 0, "color", fonte); }
        }
        s
    }

    #[test]
    fn por_cor_de_fundo_em_cima_mantem_a_ordem_do_resto() {
        let mut s = aba_coloria();
        let k = chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#FF0000".into()), em_cima: true });
        assert!(ordenar_por_linhas(&mut s, Rect::new(0, 0, 4, 1), &[k], false, false));
        assert_eq!(coluna(&s, 0, 5), ["c", "b", "a", "d", "e"], "vermelhas primeiro; o resto como estava");
        assert_eq!(coluna(&s, 1, 5), ["1", "4", "2", "3", "5"], "a linha anda inteira");
        assert_eq!(s.estilo_resolvido(1, 0).unwrap().bg.as_deref(), Some("#ff0000"), "o estilo vai junto");
        assert!(s.undo());
        assert_eq!(coluna(&s, 0, 5), ["c", "a", "d", "b", "e"], "um Ctrl+Z");
    }

    #[test]
    fn por_cor_embaixo_e_dois_niveis_de_cor() {
        let mut s = aba_coloria();
        let ks = [
            chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#00ff00".into()), em_cima: true }),
            chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#ff0000".into()), em_cima: false }),
        ];
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 4, 1), &ks, false, false);
        // verde, depois sem cor (a, e), por último vermelhas (c, b).
        assert_eq!(coluna(&s, 0, 5), ["d", "a", "e", "c", "b"]);
    }

    #[test]
    fn por_cor_da_fonte_e_sem_cor() {
        let mut s = aba_coloria();
        let k = chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fonte, cor: None, em_cima: true });
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 4, 1), &[k], false, false);
        assert_eq!(coluna(&s, 0, 5), ["c", "d", "e", "a", "b"], "fonte automática primeiro");
    }

    #[test]
    fn cabecalho_nao_anda() {
        let mut s = aba_coloria();
        let k = chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#00ff00".into()), em_cima: true });
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 4, 1), &[k], true, false);
        assert_eq!(coluna(&s, 0, 5), ["c", "d", "a", "b", "e"]);
    }

    #[test]
    fn por_lista_dias_da_semana_e_fora_da_lista() {
        let mut s = Sheet::new("T");
        for (r, v) in ["Sex", "dom", "qua", "xyz", "SEG", "Abc"].iter().enumerate() {
            s.set_cell(r as u32, 0, v);
        }
        let dias = listas_prontas()[0].1.clone();
        let asc = chave(0, CriterioOrdem::Lista(dias.clone()));
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 5, 0), &[asc], false, false);
        assert_eq!(coluna(&s, 0, 6), ["dom", "SEG", "qua", "Sex", "Abc", "xyz"], "fora da lista depois, em ordem normal");
        let desc = ChaveOrdem { indice: 0, ascendente: false, criterio: CriterioOrdem::Lista(dias) };
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 5, 0), &[desc], false, false);
        assert_eq!(coluna(&s, 0, 6), ["xyz", "Abc", "Sex", "qua", "SEG", "dom"]);
    }

    #[test]
    fn por_lista_de_meses_e_lista_dada() {
        let mut s = Sheet::new("T");
        for (r, v) in ["março", "janeiro", "dezembro", "fevereiro"].iter().enumerate() {
            s.set_cell(r as u32, 0, v);
        }
        let meses = listas_prontas()[3].1.clone();
        ordenar_por_linhas(&mut s, Rect::new(0, 0, 3, 0), &[chave(0, CriterioOrdem::Lista(meses))], false, false);
        assert_eq!(coluna(&s, 0, 4), ["janeiro", "fevereiro", "março", "dezembro"]);
        let mut t = Sheet::new("T");
        for (r, v) in ["Alta", "baixa", "Média", "alta"].iter().enumerate() {
            t.set_cell(r as u32, 0, v);
        }
        let lista = lista_de_texto("baixa; média; alta");
        ordenar_por_linhas(&mut t, Rect::new(0, 0, 3, 0), &[chave(0, CriterioOrdem::Lista(lista))], false, false);
        assert_eq!(coluna(&t, 0, 4), ["baixa", "Média", "Alta", "alta"]);
    }

    #[test]
    fn da_esquerda_para_a_direita_por_valor() {
        let mut s = Sheet::new("T");
        // Linha 0: rótulos de coluna; coluna A: rótulos de linha.
        let dados = [["", "z", "b", "m"], ["x", "30", "10", "20"], ["y", "3", "1", "2"]];
        for (r, linha) in dados.iter().enumerate() {
            for (c, v) in linha.iter().enumerate() {
                if !v.is_empty() { s.set_cell(r as u32, c as u32, v); }
            }
        }
        s.set_cell(3, 2, "=C2*2"); // fórmula na coluna "b" (valor 10), olhando a própria coluna
        // Ordena pela linha 1 (valores), ascendente; coluna A fica.
        let k = ChaveOrdem { indice: 1, ascendente: true, criterio: CriterioOrdem::Valor };
        assert!(ordenar_por_colunas(&mut s, Rect::new(0, 0, 3, 3), &[k], true, false));
        let linha = |r: u32| (0..4).map(|c| crate::operacoes::filtro::texto_js(&s.get_value(r, c))).collect::<Vec<_>>();
        assert_eq!(linha(0), ["", "b", "m", "z"]);
        assert_eq!(linha(1), ["x", "10", "20", "30"]);
        assert_eq!(linha(2), ["y", "1", "2", "3"]);
        // A fórmula foi de C4 para B4 junto com a coluna "b" e ainda olha a própria coluna.
        assert_eq!(s.get_value(3, 1), Value::Number(20.0));
        assert!(s.undo());
        assert_eq!(crate::operacoes::filtro::texto_js(&s.get_value(0, 1)), "z");
        assert_eq!(crate::operacoes::filtro::texto_js(&s.get_value(1, 2)), "10", "um Ctrl+Z desfaz tudo");
    }

    #[test]
    fn da_esquerda_para_a_direita_por_cor() {
        let mut s = Sheet::new("T");
        for (c, v) in ["a", "b", "c", "d"].iter().enumerate() {
            s.set_cell(0, c as u32, v);
        }
        pintar(&mut s, 0, 1, "bg", "#ffff00");
        pintar(&mut s, 0, 3, "bg", "#ffff00");
        let k = chave(0, CriterioOrdem::Cor { alvo: AlvoCor::Fundo, cor: Some("#ffff00".into()), em_cima: true });
        ordenar_por_colunas(&mut s, Rect::new(0, 0, 0, 3), &[k], false, false);
        let linha: Vec<_> = (0..4).map(|c| crate::operacoes::filtro::texto_js(&s.get_value(0, c))).collect();
        assert_eq!(linha, ["b", "d", "a", "c"]);
        assert_eq!(s.estilo_resolvido(0, 0).unwrap().bg.as_deref(), Some("#ffff00"));
        assert!(s.estilo_resolvido(0, 2).and_then(|e| e.bg).is_none());
    }

    #[test]
    fn sem_mudanca_devolve_nada() {
        let mut s = Sheet::new("T");
        s.set_cell(0, 0, "a");
        s.set_cell(1, 0, "b");
        let k = chave(0, CriterioOrdem::Valor);
        assert!(!ordenar_por_linhas(&mut s, Rect::new(0, 0, 1, 0), &[k], false, false));
    }
}
