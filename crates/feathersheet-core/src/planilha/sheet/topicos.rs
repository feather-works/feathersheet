//! Estrutura de tópicos do Excel (Dados → Agrupar / Desagrupar / Mostrar
//! detalhe / Ocultar detalhe e os botões de nível 1, 2, 3… ao lado da grade).
//!
//! Cada linha (coluna) tem um nível de 0 a [`NIVEL_MAXIMO`]. Um **grupo** de
//! nível `n` é uma sequência contígua de índices com nível `>= n`; o resumo
//! dele é o índice logo depois (abaixo, nas linhas; à direita, nas colunas),
//! como o padrão do Excel. Recolher oculta o grupo (tamanho zero, o mesmo
//! estado do Ocultar linhas) e marca o resumo em `recolhidas`, para que
//! expandir um grupo de fora não abra os de dentro que estavam recolhidos.
//!
//! Tudo entra no desfazer: o passo dos níveis e os passos de dimensão andam
//! na mesma transação.
use std::collections::{BTreeMap, BTreeSet};

use super::Sheet;
use crate::celulas::refs::{MAX_COLS, MAX_ROWS};

/// O Excel aceita sete níveis de agrupamento (oito botões, contando o 1).
pub const NIVEL_MAXIMO: u8 = 7;

/// Acima disto, agrupar ou recolher de uma vez travaria a interface e
/// encheria o desfazer de passos de dimensão.
const LIMITE_INDICES: u32 = 200_000;

/// Os níveis de um eixo e os resumos recolhidos.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct EixoTopicos {
    pub niveis: BTreeMap<u32, u8>,
    /// O índice do resumo de cada grupo recolhido.
    pub recolhidas: BTreeSet<u32>,
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Topicos {
    pub linhas: EixoTopicos,
    pub colunas: EixoTopicos,
}

impl Topicos {
    pub fn vazio(&self) -> bool {
        self.linhas.niveis.is_empty() && self.colunas.niveis.is_empty()
    }

    pub fn eixo(&self, coluna: bool) -> &EixoTopicos {
        if coluna { &self.colunas } else { &self.linhas }
    }

    fn eixo_mut(&mut self, coluna: bool) -> &mut EixoTopicos {
        if coluna { &mut self.colunas } else { &mut self.linhas }
    }

    /// Desloca os níveis depois de inserir (`removendo == false`) ou excluir
    /// `count` linhas/colunas em `at`.
    pub fn deslocar(&mut self, coluna: bool, at: u32, count: u32, removendo: bool) {
        let eixo = self.eixo_mut(coluna);
        let mover = |p: u32| -> Option<u32> {
            if !removendo { return Some(if p >= at { p + count } else { p }); }
            if p >= at && p < at + count { return None; }
            Some(if p >= at + count { p - count } else { p })
        };
        eixo.niveis = std::mem::take(&mut eixo.niveis).into_iter()
            .filter_map(|(i, n)| mover(i).map(|i| (i, n))).collect();
        eixo.recolhidas = std::mem::take(&mut eixo.recolhidas).into_iter()
            .filter_map(mover).collect();
    }
}

impl EixoTopicos {
    pub fn nivel(&self, indice: u32) -> u8 {
        self.niveis.get(&indice).copied().unwrap_or(0)
    }

    pub fn nivel_maior(&self) -> u8 {
        self.niveis.values().copied().max().unwrap_or(0)
    }

    /// O grupo de nível `nivel` que contém `indice`: `(início, fim)`.
    fn grupo_em(&self, indice: u32, nivel: u8) -> Option<(u32, u32)> {
        if self.nivel(indice) < nivel { return None; }
        let mut inicio = indice;
        while inicio > 0 && self.nivel(inicio - 1) >= nivel { inicio -= 1; }
        let mut fim = indice;
        while self.nivel(fim + 1) >= nivel { fim += 1; }
        Some((inicio, fim))
    }

    /// O grupo cujo resumo é `indice`, do nível mais de dentro para fora:
    /// `(nível, início, fim)`.
    pub fn grupo_do_resumo(&self, indice: u32, nivel: u8) -> Option<(u32, u32)> {
        if indice == 0 || self.nivel(indice) >= nivel { return None; }
        self.grupo_em(indice - 1, nivel)
    }

    /// O resumo de nível `nivel` que vale para `indice`: o próprio índice se
    /// for resumo, senão o do grupo que o contém.
    fn resumo_para(&self, indice: u32, nivel: u8) -> Option<u32> {
        if self.grupo_do_resumo(indice, nivel).is_some() { return Some(indice); }
        self.grupo_em(indice, nivel).map(|(_, fim)| fim + 1)
    }

    /// Esquece as marcas de recolhido que não são mais resumo de nada.
    fn limpar_recolhidas(&mut self) {
        let validas: BTreeSet<u32> = self.recolhidas.iter().copied()
            .filter(|&s| s > 0 && self.nivel(s - 1) > self.nivel(s)).collect();
        self.recolhidas = validas;
    }
}

impl Sheet {
    fn limite_eixo(coluna: bool) -> u32 { if coluna { MAX_COLS } else { MAX_ROWS } }

    fn oculto(&self, coluna: bool, indice: u32) -> bool {
        let mapa = if coluna { &self.col_widths } else { &self.row_heights };
        mapa.get(&indice) == Some(&0)
    }

    /// Mostra ou oculta os índices, registrando cada mudança na transação
    /// aberta. Mostrar devolve o tamanho padrão, como o Reexibir.
    fn trocar_visibilidade(&mut self, coluna: bool, indices: impl IntoIterator<Item = u32>, ocultar: bool) {
        for i in indices {
            if self.oculto(coluna, i) == ocultar { continue; }
            self.registrar_dimensao(coluna, i, if ocultar { Some(0) } else { None });
        }
    }

    /// Troca os níveis por `novo` numa transação, com os passos de dimensão
    /// que `depois` fizer na mesma transação.
    fn mudar_topicos(&mut self, novo: Topicos, depois: impl FnOnce(&mut Self)) {
        self.abrir_transacao();
        self.registrar_topicos(novo);
        depois(self);
        self.fechar_transacao();
    }

    /// Agrupa os índices `a..=b`: cada um sobe um nível (Shift+Alt+→).
    pub fn agrupar(&mut self, coluna: bool, a: u32, b: u32) -> Result<(), String> {
        let (a, b) = (a.min(b), a.max(b));
        if b >= Self::limite_eixo(coluna) { return Err(crate::tr!("O intervalo ultrapassa os limites da planilha.").into()); }
        if b - a + 1 > LIMITE_INDICES { return Err(crate::tr!("Seleção grande demais para agrupar.").into()); }
        let eixo = self.topicos.eixo(coluna);
        if (a..=b).any(|i| eixo.nivel(i) >= NIVEL_MAXIMO) {
            return Err(crate::trf!("O Excel permite no máximo {NIVEL_MAXIMO} níveis de agrupamento."));
        }
        let mut novo = self.topicos.clone();
        let eixo = novo.eixo_mut(coluna);
        for i in a..=b { *eixo.niveis.entry(i).or_insert(0) += 1; }
        eixo.limpar_recolhidas();
        self.mudar_topicos(novo, |_| {});
        Ok(())
    }

    /// Desagrupa `a..=b`: cada índice agrupado desce um nível
    /// (Shift+Alt+←). Devolve quantos mudaram; os ocultos continuam ocultos,
    /// como no Excel.
    pub fn desagrupar(&mut self, coluna: bool, a: u32, b: u32) -> Result<usize, String> {
        let (a, b) = (a.min(b), a.max(b));
        let mut novo = self.topicos.clone();
        let eixo = novo.eixo_mut(coluna);
        let alvo: Vec<u32> = eixo.niveis.range(a..=b).map(|(&i, _)| i).collect();
        if alvo.is_empty() { return Ok(0); }
        for &i in &alvo {
            let n = eixo.niveis.get_mut(&i).expect("listado acima");
            *n -= 1;
            if *n == 0 { eixo.niveis.remove(&i); }
        }
        eixo.limpar_recolhidas();
        self.mudar_topicos(novo, |_| {});
        Ok(alvo.len())
    }

    /// Apaga toda a estrutura do eixo e reexibe o que ela tinha recolhido.
    pub fn limpar_estrutura(&mut self, coluna: bool) -> bool {
        if self.topicos.eixo(coluna).niveis.is_empty() { return false; }
        let indices: Vec<u32> = self.topicos.eixo(coluna).niveis.keys().copied().collect();
        let mut novo = self.topicos.clone();
        *novo.eixo_mut(coluna) = EixoTopicos::default();
        self.mudar_topicos(novo, |s| s.trocar_visibilidade(coluna, indices, false));
        true
    }

    /// Recolhe (Ocultar detalhe) o grupo mais de dentro que contém `indice`
    /// ou que tem `indice` como resumo.
    pub fn ocultar_detalhe(&mut self, coluna: bool, indice: u32) -> Result<(), String> {
        let eixo = self.topicos.eixo(coluna);
        let resumo = (1..=eixo.nivel_maior()).rev()
            .find_map(|n| eixo.resumo_para(indice, n).filter(|s| !eixo.recolhidas.contains(s)).map(|s| (n, s)));
        let Some((nivel, resumo)) = resumo else { return Err(crate::tr!("Não há grupo aberto aqui.").into()); };
        self.recolher(coluna, nivel, resumo, true)
    }

    /// Expande (Mostrar detalhe) o grupo recolhido que tem `indice` como
    /// resumo ou que o contém.
    pub fn mostrar_detalhe(&mut self, coluna: bool, indice: u32) -> Result<(), String> {
        let eixo = self.topicos.eixo(coluna);
        let resumo = (1..=eixo.nivel_maior())
            .find_map(|n| eixo.resumo_para(indice, n).filter(|s| eixo.recolhidas.contains(s)).map(|s| (n, s)));
        let Some((nivel, resumo)) = resumo else { return Err(crate::tr!("Não há grupo recolhido aqui.").into()); };
        self.recolher(coluna, nivel, resumo, false)
    }

    /// O clique no [+]/[−] do grupo de nível `nivel` cujo resumo é `resumo`.
    pub fn alternar_grupo(&mut self, coluna: bool, nivel: u8, resumo: u32) -> Result<(), String> {
        let recolhido = self.topicos.eixo(coluna).recolhidas.contains(&resumo);
        self.recolher(coluna, nivel, resumo, !recolhido)
    }

    fn recolher(&mut self, coluna: bool, nivel: u8, resumo: u32, recolher: bool) -> Result<(), String> {
        let eixo = self.topicos.eixo(coluna);
        let (inicio, fim) = eixo.grupo_do_resumo(resumo, nivel).ok_or_else(|| crate::tr!("Não há grupo aqui."))?;
        if fim - inicio + 1 > LIMITE_INDICES { return Err(crate::tr!("Grupo grande demais para recolher.").into()); }
        let mut novo = self.topicos.clone();
        let mostrar = if recolher {
            novo.eixo_mut(coluna).recolhidas.insert(resumo);
            Vec::new()
        } else {
            novo.eixo_mut(coluna).recolhidas.remove(&resumo);
            let mut v = Vec::new();
            visiveis_ao_expandir(novo.eixo(coluna), nivel, inicio, fim, &mut v);
            v
        };
        self.mudar_topicos(novo, |s| {
            if recolher { s.trocar_visibilidade(coluna, inicio..=fim, true); }
            else { s.trocar_visibilidade(coluna, mostrar, false); }
        });
        Ok(())
    }

    /// Os botões 1, 2, 3… : mostra só até o nível `nivel` (1 = só os
    /// totais de fora). Os grupos de nível `>= nivel` ficam recolhidos.
    pub fn mostrar_nivel(&mut self, coluna: bool, nivel: u8) -> Result<(), String> {
        let eixo = self.topicos.eixo(coluna);
        if eixo.niveis.is_empty() { return Err(crate::tr!("Não há estrutura de tópicos.").into()); }
        if eixo.niveis.len() as u32 > LIMITE_INDICES { return Err("Estrutura grande demais.".into()); }
        let nivel = nivel.max(1);
        let mut novo = self.topicos.clone();
        let e = novo.eixo_mut(coluna);
        e.recolhidas = e.niveis.iter()
            .filter(|&(&i, &n)| n >= nivel && e.nivel(i + 1) < n)
            .map(|(&i, _)| i + 1).collect();
        let (ocultar, mostrar): (Vec<_>, Vec<_>) = e.niveis.iter()
            .map(|(&i, &n)| (i, n)).partition(|&(_, n)| n >= nivel);
        self.mudar_topicos(novo, |s| {
            s.trocar_visibilidade(coluna, ocultar.into_iter().map(|(i, _)| i), true);
            s.trocar_visibilidade(coluna, mostrar.into_iter().map(|(i, _)| i), false);
        });
        Ok(())
    }
}

/// Os índices que aparecem ao expandir o grupo `inicio..=fim` de nível
/// `nivel`: os do próprio nível e, dos grupos de dentro, só os que não
/// estão recolhidos.
fn visiveis_ao_expandir(eixo: &EixoTopicos, nivel: u8, inicio: u32, fim: u32, saida: &mut Vec<u32>) {
    let mut i = inicio;
    while i <= fim {
        if eixo.nivel(i) <= nivel {
            saida.push(i);
            i += 1;
            continue;
        }
        let mut j = i;
        while j < fim && eixo.nivel(j + 1) > nivel { j += 1; }
        if !eixo.recolhidas.contains(&(j + 1)) {
            visiveis_ao_expandir(eixo, nivel + 1, i, j, saida);
        }
        i = j + 1;
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    fn oculta(s: &Sheet, r: u32) -> bool { s.linha_oculta(r) }

    #[test]
    fn agrupar_recolher_e_expandir_desfazem_num_passo() {
        let mut s = Sheet::new("A");
        s.set_cell(0, 0, "x");
        s.clear_history();
        s.agrupar(false, 1, 3).unwrap();
        assert_eq!(s.topicos.linhas.nivel(2), 1);
        assert_eq!(s.topicos.linhas.nivel(4), 0);
        // O [−] fica no resumo, a linha 4.
        s.alternar_grupo(false, 1, 4).unwrap();
        assert!((1..=3).all(|r| oculta(&s, r)));
        assert!(!oculta(&s, 4));
        assert!(s.undo(), "um Ctrl+Z reabre");
        assert!((1..=3).all(|r| !oculta(&s, r)));
        assert!(s.topicos.linhas.recolhidas.is_empty());
        assert!(s.undo(), "outro desfaz o agrupar");
        assert!(s.topicos.vazio());
        assert!(s.redo() && s.redo());
        assert!(oculta(&s, 2));
        s.mostrar_detalhe(false, 4).unwrap();
        assert!(!oculta(&s, 2));
    }

    #[test]
    fn expandir_o_de_fora_mantem_o_de_dentro_recolhido() {
        let mut s = Sheet::new("A");
        s.agrupar(false, 1, 8).unwrap();
        s.agrupar(false, 2, 4).unwrap();
        s.alternar_grupo(false, 2, 5).unwrap();
        s.alternar_grupo(false, 1, 9).unwrap();
        assert!((1..=8).all(|r| oculta(&s, r)));
        s.alternar_grupo(false, 1, 9).unwrap();
        assert!(!oculta(&s, 1) && !oculta(&s, 5) && !oculta(&s, 8));
        assert!((2..=4).all(|r| oculta(&s, r)), "o grupo de dentro continua recolhido");
    }

    #[test]
    fn botoes_de_nivel_e_limites() {
        let mut s = Sheet::new("A");
        s.agrupar(false, 0, 5).unwrap();
        s.agrupar(false, 1, 2).unwrap();
        s.mostrar_nivel(false, 2).unwrap();
        assert!(oculta(&s, 1) && oculta(&s, 2) && !oculta(&s, 0) && !oculta(&s, 3));
        s.mostrar_nivel(false, 1).unwrap();
        assert!((0..=5).all(|r| oculta(&s, r)));
        s.mostrar_nivel(false, 3).unwrap();
        assert!((0..=5).all(|r| !oculta(&s, r)));
        for _ in 0..5 { s.agrupar(false, 1, 1).unwrap(); }
        assert!(s.agrupar(false, 1, 1).unwrap_err().contains("7 níveis"));
        assert!(s.agrupar(false, 0, MAX_ROWS).is_err());
    }

    #[test]
    fn desagrupar_colunas_e_deslocar() {
        let mut s = Sheet::new("A");
        s.agrupar(true, 2, 4).unwrap();
        assert_eq!(s.desagrupar(true, 3, 9).unwrap(), 2);
        assert_eq!(s.topicos.colunas.nivel(2), 1);
        assert_eq!(s.topicos.colunas.nivel(3), 0);
        s.topicos.deslocar(true, 0, 2, false);
        assert_eq!(s.topicos.colunas.nivel(4), 1);
        assert!(s.limpar_estrutura(true));
        assert!(s.topicos.vazio());
    }
}
