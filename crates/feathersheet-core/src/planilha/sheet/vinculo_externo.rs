//! As fórmulas que leem outro arquivo (`=[Vendas.fshz]Dados!A1`), do lado
//! da aba: quem suja quando chega um valor de fora, e o `#REF!` do intervalo
//! de um arquivo (ou aba) sem valores.
//!
//! Essas fórmulas são sempre avulsas (`pattern_check` não as empacota) e
//! moram em `externas`, junto das que leem outra aba — são poucas, e é por
//! elas que a atualização acha quem lê cada célula de fora.

use super::*;
use crate::planilha::vinculo;

impl Sheet {
    /// Suja as fórmulas desta aba que leem `arquivo` (na `aba`, ou em
    /// qualquer aba com `None`) em alguma célula de `rects` (`None`: em
    /// qualquer célula), e quem depende delas. Responde se sujou alguma.
    pub(crate) fn sujar_vinculo(&mut self, arquivo: &str, aba: Option<&str>, rects: Option<&[Rect]>) -> bool {
        if self.externas.is_empty() {
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
        if raizes.is_empty() {
            return false;
        }
        raizes.sort_unstable();
        self.tocar();
        // O que a fórmula de fora vai dar só se sabe calculando.
        self.rastro.borrow_mut().desconhecer();
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
