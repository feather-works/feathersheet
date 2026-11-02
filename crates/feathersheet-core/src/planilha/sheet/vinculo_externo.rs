//! As fórmulas que leem outro arquivo (`=[Vendas.fshz]Dados!A1`), do lado
//! da aba: quem suja quando chega um valor de fora, e o `#REF!` do intervalo
//! de um arquivo (ou aba) sem valores.
//!
//! As avulsas moram em `externas`, junto das que leem outra aba; as empacotadas (desde #136)
//! são achadas pelas referências do padrão de cada coluna (`vinculos_empacotados`).

use super::*;
use crate::planilha::vinculo;

/// Acima disto, o padrão de vínculo que muda suja a coluna inteira em vez de linha a linha.
const LINHAS_PARA_A_COLUNA: u32 = 1_000;

impl Sheet {
    /// As faixas de outro arquivo que as fórmulas empacotadas leem: `(coluna, aba como a
    /// fórmula escreveu, área)`. Por padrão, a faixa da primeira e a da última linha em que ele
    /// aparece, juntas (a relativa anda entre elas; a absoluta é a mesma).
    pub(crate) fn vinculos_empacotados(&self) -> Vec<(u32, String, Rect)> {
        self.vinculos_empacotados_por_padrao().into_iter().map(|(col, _, nome, area)| (col, nome, area)).collect()
    }

    /// [`Self::vinculos_empacotados`] com o padrão: `(coluna, (primeira, última, id), aba, área)`.
    fn vinculos_empacotados_por_padrao(&self) -> Vec<(u32, (u32, u32, usize), String, Rect)> {
        let faixas_em = |r: u32, col: u32| -> Vec<(String, Rect)> {
            let mut v = Vec::new();
            let Some(raw) = self.formulas.raw_at(r, col) else { return v };
            let Ok(no) = parse_formula(raw.strip_prefix('=').unwrap_or(&raw)) else { return v };
            no.walk(&mut |n| match n {
                Node::Ref { sheet: Some(s), row, col } if vinculo::e_externa(s) => v.push((s.clone(), Rect::new(*row, *col, *row, *col))),
                Node::Range { sheet: Some(s), r0, c0, r1, c1 } if vinculo::e_externa(s) => {
                    v.push((s.clone(), Rect::new(*r0.min(r1), *c0.min(c1), *r0.max(r1), *c0.max(c1))));
                }
                _ => {}
            });
            v
        };
        let mut saida = Vec::new();
        for (col, primeira, ultima, id) in self.formulas.padroes_com_vinculo() {
            let padrao = (primeira, ultima, id);
            let (cima, baixo) = (faixas_em(primeira, col), faixas_em(ultima, col));
            if cima.len() == baixo.len() {
                for ((nome, a), (_, b)) in cima.into_iter().zip(baixo) {
                    saida.push((col, padrao, nome, Rect::new(a.r0.min(b.r0), a.c0.min(b.c0), a.r1.max(b.r1), a.c1.max(b.c1))));
                }
            } else {
                saida.extend(cima.into_iter().chain(baixo).map(|(n, a)| (col, padrao, n, a)));
            }
        }
        saida
    }

    /// Suja as fórmulas desta aba que leem `arquivo` (na `aba`, ou em
    /// qualquer aba com `None`) em alguma célula de `rects` (`None`: em
    /// qualquer célula), e quem depende delas. Responde se sujou alguma.
    pub(crate) fn sujar_vinculo(&mut self, arquivo: &str, aba: Option<&str>, rects: Option<&[Rect]>) -> bool {
        let empacotados = self.vinculos_empacotados_por_padrao();
        if self.externas.is_empty() && empacotados.is_empty() {
            return false;
        }
        let chave = vinculo::chave_arquivo(arquivo);
        let casa_aba = |nome: &str| -> bool {
            vinculo::separar(nome).is_some_and(|(arq, a)| {
                vinculo::chave_arquivo(&arq) == chave && aba.is_none_or(|x| mesmo_nome(x, a))
            })
        };
        let toca = |r0: u32, c0: u32, r1: u32, c1: u32| {
            rects.is_none_or(|rs| rs.iter().any(|r| r.r0 <= r1 && r0 <= r.r1 && r.c0 <= c1 && c0 <= r.c1))
        };
        let mut raizes: Vec<CellKey> = Vec::new();
        for &k in &self.externas {
            let Some(celula) = self.cells.get(&k) else { continue };
            let arvore;
            let no = match &celula.formula {
                Some(n) => n,
                None => match celula.raw.strip_prefix('=').map(parse_formula) {
                    Some(Ok(n)) => {
                        arvore = n;
                        &arvore
                    }
                    _ => continue,
                },
            };
            let mut le = false;
            no.walk(&mut |n| match n {
                Node::Ref { sheet: Some(s), row, col } if casa_aba(s) => le |= toca(*row, *col, *row, *col),
                Node::Range { sheet: Some(s), r0, c0, r1, c1 } if casa_aba(s) => {
                    le |= toca(*r0.min(r1), *c0.min(c1), *r0.max(r1), *c0.max(c1));
                }
                _ => {}
            });
            if le {
                raizes.push(k);
            }
        }
        // Empacotadas: as linhas do padrão que lê o arquivo dentro de `rects`; padrão de muitas
        // linhas suja a coluna inteira de uma vez (uma cascata por célula era o #136).
        let mut colunas: std::collections::HashSet<u32> = std::collections::HashSet::new();
        for (col, (primeira, ultima, id), _, _) in empacotados.iter()
            .filter(|(_, _, nome, a)| casa_aba(nome) && toca(a.r0, a.c0, a.r1, a.c1))
        {
            if ultima - primeira >= LINHAS_PARA_A_COLUNA {
                colunas.insert(*col);
            } else {
                raizes.extend(self.formulas.linhas_do_padrao(*col, *primeira, *ultima, *id).into_iter().map(|r| key(r, *col)));
            }
        }
        if raizes.is_empty() && colunas.is_empty() {
            return false;
        }
        raizes.sort_unstable();
        self.tocar();
        // O que a fórmula de fora vai dar só se sabe calculando.
        self.rastro.borrow_mut().desconhecer();
        if !colunas.is_empty() {
            let usos = self.formulas.columns_in_use();
            self.invalidar_colunas_e_dependentes(colunas, &usos);
        }
        // Uma cascata para todas as raízes (uma por fórmula levava minutos no #136).
        self.adiar_cascata();
        for k in raizes {
            self.mark_dirty_cascade(k);
        }
        self.soltar_cascata();
        true
    }

    /// `#REF!` quando a fórmula lê um **intervalo** de outro arquivo que não
    /// tem valores (nunca carregado) ou não tem aquela aba. A célula avulsa
    /// já dá `#REF!` sozinha; o intervalo, não — a `SOMA` de nada seria 0, e
    /// um zero calado no lugar de "a origem não está aqui" engana.
    pub(super) fn intervalo_externo_sem_valores(&self, no: &Node, host: Option<&dyn Hospedeiro>) -> Option<Value> {
        let host = host?;
        let mut falta: Option<String> = None;
        no.walk(&mut |n| {
            if let Node::Range { sheet: Some(s), .. } = n {
                if falta.is_none() && vinculo::e_externa(s) && host.aba(s).is_none() {
                    falta = Some(s.clone());
                }
            }
        });
        falta.map(|nome| {
            Value::Error(CellError::com_motivo(ErrorCode::Ref, format!("Sem valores de \"{nome}\": abra o arquivo ou atualize os valores")))
        })
    }
}
