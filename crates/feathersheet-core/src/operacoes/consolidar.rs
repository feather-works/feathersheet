//! Dados → Consolidar, como o do Excel: junta vários intervalos (de abas
//! diferentes, em geral) numa tabela só, com uma conta por célula. Sem
//! rótulos, casa pela posição; com rótulos na linha de cima e/ou na coluna
//! da esquerda, casa pelo texto do rótulo (sem ligar para maiúscula), na
//! ordem em que cada um aparece primeiro.

use crate::celulas::values::{to_text, Value};
use crate::dinamica::acumulador::{Acumulador, Medida};
use crate::dinamica::pivot::AgregacaoOp;

/// Os intervalos de origem já lidos (linha a linha) e a conta.
pub struct Consolidar<'a> {
    pub origens: &'a [Vec<Vec<Value>>],
    pub op: AgregacaoOp,
    pub rotulos_topo: bool,
    pub rotulos_esquerda: bool,
}

/// Os rótulos na ordem em que apareceram, com o índice de cada um pelo texto
/// (a busca linear travava com centenas de milhares de rótulos diferentes).
#[derive(Default)]
struct Rotulos {
    valores: Vec<Value>,
    indice: std::collections::HashMap<String, usize>,
}

/// Índice de um rótulo na lista (criado no fim se é novo).
fn posicao(lista: &mut Rotulos, v: &Value) -> usize {
    let proximo = lista.valores.len();
    let i = *lista.indice.entry(to_text(v).trim().to_lowercase()).or_insert(proximo);
    if i == proximo { lista.valores.push(v.clone()); }
    i
}

impl Consolidar<'_> {
    /// A tabela consolidada, com os rótulos nas bordas quando pedidos (o
    /// canto de cima à esquerda fica vazio, como no Excel). Célula sem nenhum
    /// valor fica vazia.
    pub fn montar(&self) -> Vec<Vec<Value>> {
        let (dr, dc) = (self.rotulos_topo as usize, self.rotulos_esquerda as usize);
        let mut linhas = Rotulos::default();
        let mut colunas = Rotulos::default();
        let mut celulas: Vec<Vec<Option<Acumulador>>> = Vec::new();
        for origem in self.origens {
            let largura = origem.iter().map(Vec::len).max().unwrap_or(0);
            // Coluna da origem → coluna do resultado.
            let mapa_col: Vec<usize> = (dc..largura).map(|c| if self.rotulos_topo {
                posicao(&mut colunas, origem.first().and_then(|l| l.get(c)).unwrap_or(&Value::Empty))
            } else { c - dc }).collect();
            for (r, linha) in origem.iter().enumerate().skip(dr) {
                let lr = if self.rotulos_esquerda {
                    posicao(&mut linhas, linha.first().unwrap_or(&Value::Empty))
                } else { r - dr };
                for (k, &lc) in mapa_col.iter().enumerate() {
                    let Some(v) = linha.get(k + dc) else { continue };
                    let m = Medida::de(v);
                    if matches!(m, Medida::Vazio) { continue; }
                    if celulas.len() <= lr { celulas.resize_with(lr + 1, Vec::new); }
                    let fila = &mut celulas[lr];
                    if fila.len() <= lc { fila.resize_with(lc + 1, || None); }
                    fila[lc].get_or_insert_with(|| Acumulador::novo(self.op)).receber_medida(m);
                }
            }
        }
        let altura = if self.rotulos_esquerda { linhas.valores.len() } else { celulas.len() };
        let largura = if self.rotulos_topo { colunas.valores.len() } else { celulas.iter().map(Vec::len).max().unwrap_or(0) };
        let mut saida = Vec::with_capacity(altura + dr);
        if self.rotulos_topo {
            let mut topo = vec![Value::Empty; dc];
            topo.extend(colunas.valores.iter().cloned());
            saida.push(topo);
        }
        for r in 0..altura {
            let mut linha = Vec::with_capacity(largura + dc);
            if self.rotulos_esquerda { linha.push(linhas.valores[r].clone()); }
            for c in 0..largura {
                linha.push(match celulas.get(r).and_then(|f| f.get(c)).and_then(Option::as_ref) {
                    Some(a) => Value::Number(a.resultado()),
                    None => Value::Empty,
                });
            }
            saida.push(linha);
        }
        saida
    }
}

impl crate::planilha::workbook::Workbook {
    /// Consolida os intervalos `origens` (cada `Rect` com a aba em `sheet`; sem aba = `destino`)
    /// e grava a partir de (`linha`, `coluna`) da aba `destino`, num Ctrl+Z. Recusa se for
    /// escrever por cima de alguma célula com conteúdo. Devolve o tamanho gravado.
    pub fn consolidar(&mut self, origens: &[crate::celulas::refs::Rect], op: AgregacaoOp, rotulos_topo: bool,
        rotulos_esquerda: bool, destino: usize, linha: u32, coluna: u32) -> Result<(u32, u32), String> {
        if origens.is_empty() { return Err(crate::tr!("Escolha pelo menos um intervalo para consolidar.").into()); }
        let mut lidas = Vec::with_capacity(origens.len());
        for r in origens {
            let aba = match r.sheet.as_deref() {
                Some(nome) => self.indice_por_nome(nome).ok_or_else(|| crate::trf!("A aba \"{nome}\" não existe."))?,
                None => destino,
            };
            // Teto de leitura: Consolidar é para tabelas de resumo, não para colunas inteiras.
            let celulas = (r.r1 - r.r0 + 1) as u64 * (r.c1 - r.c0 + 1) as u64;
            if celulas > 2_000_000 { return Err(crate::tr!("Intervalo grande demais para consolidar (até 2 milhões de células cada).").into()); }
            lidas.push((r.r0..=r.r1).map(|l| (r.c0..=r.c1).map(|c| self.valor(aba, l, c)).collect()).collect::<Vec<Vec<Value>>>());
        }
        let tabela = Consolidar { origens: &lidas, op, rotulos_topo, rotulos_esquerda }.montar();
        let altura = tabela.len() as u32;
        let largura = tabela.iter().map(Vec::len).max().unwrap_or(0) as u32;
        if altura == 0 || largura == 0 { return Err(crate::tr!("Os intervalos não têm valores para consolidar.").into()); }
        let (r1, c1) = (linha.checked_add(altura - 1), coluna.checked_add(largura - 1));
        let (Some(r1), Some(c1)) = (r1, c1) else { return Err(crate::tr!("O resultado passa do fim da planilha.").into()) };
        if r1 >= crate::celulas::refs::MAX_ROWS || c1 >= crate::celulas::refs::MAX_COLS {
            return Err(crate::tr!("O resultado passa do fim da planilha.").into());
        }
        let aba = self.aba(destino).ok_or_else(|| crate::tr!("Aba de destino inexistente."))?;
        let ocupada = (linha..=r1).any(|l| (coluna..=c1).any(|c| !aba.sheet.get_raw(l, c).is_empty()));
        if ocupada { return Err(crate::tr!("O destino tem dados: escolha uma área vazia.").into()); }
        self.editar(destino, |s| {
            s.abrir_transacao();
            for (i, fila) in tabela.iter().enumerate() {
                for (j, v) in fila.iter().enumerate() {
                    if v.is_empty() { continue; }
                    // Rótulo da origem é dado, nunca fórmula a executar no destino.
                    let texto = match v { Value::Text(t) if t.starts_with('=') => format!("'{t}"), _ => to_text(v) };
                    s.set_cell(linha + i as u32, coluna + j as u32, &texto);
                }
            }
            s.fechar_transacao();
        });
        Ok((altura, largura))
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    fn t(s: &str) -> Value { Value::Text(s.into()) }
    fn n(x: f64) -> Value { Value::Number(x) }

    #[test]
    fn por_posicao_soma_celula_a_celula() {
        let jan = vec![vec![n(1.), n(2.)], vec![n(3.), n(4.)]];
        let fev = vec![vec![n(10.), n(20.)], vec![n(30.)]];
        let r = Consolidar { origens: &[jan, fev], op: AgregacaoOp::Soma, rotulos_topo: false, rotulos_esquerda: false }.montar();
        assert_eq!(r, vec![vec![n(11.), n(22.)], vec![n(33.), n(4.)]]);
    }

    #[test]
    fn por_rotulos_junta_linhas_e_colunas_pelo_nome() {
        let jan = vec![vec![Value::Empty, t("Qtd"), t("Valor")], vec![t("Maçã"), n(2.), n(10.)], vec![t("Pera"), n(1.), n(5.)]];
        let fev = vec![vec![Value::Empty, t("Valor")], vec![t("pera"), n(7.)], vec![t("Uva"), n(3.)]];
        let r = Consolidar { origens: &[jan, fev], op: AgregacaoOp::Soma, rotulos_topo: true, rotulos_esquerda: true }.montar();
        assert_eq!(r, vec![
            vec![Value::Empty, t("Qtd"), t("Valor")],
            vec![t("Maçã"), n(2.), n(10.)],
            vec![t("Pera"), n(1.), n(12.)],
            vec![t("Uva"), Value::Empty, n(3.)],
        ]);
        let media = Consolidar { origens: &[vec![vec![t("A"), n(2.)]], vec![vec![t("a"), n(4.)]]], op: AgregacaoOp::Media,
            rotulos_topo: false, rotulos_esquerda: true }.montar();
        assert_eq!(media, vec![vec![t("A"), n(3.)]]);
    }

    #[test]
    fn na_pasta_le_as_abas_grava_e_nao_apaga_dados() {
        use crate::celulas::refs::Rect;
        let mut w = crate::planilha::workbook::Workbook::new();
        for (i, nome) in ["Jan", "Fev"].iter().enumerate() {
            w.criar_aba(*nome);
            w.escrever_celula(i + 1, 0, 0, "Sul".into());
            w.escrever_celula(i + 1, 0, 1, format!("{}", (i + 1) * 10));
        }
        let fonte = |aba: &str| Rect { sheet: Some(aba.into()), ..Rect::new(0, 0, 0, 1) };
        let origens = [fonte("Jan"), fonte("Fev")];
        assert_eq!(w.consolidar(&origens, AgregacaoOp::Soma, false, true, 0, 2, 3), Ok((1, 2)));
        assert_eq!((w.valor(0, 2, 3), w.valor(0, 2, 4)), (t("Sul"), n(30.)));
        assert!(w.consolidar(&origens, AgregacaoOp::Soma, false, true, 0, 2, 3).is_err(), "não escreve por cima");
        assert!(w.consolidar(&[fonte("Dez")], AgregacaoOp::Soma, false, true, 0, 9, 9).is_err());
    }
}
