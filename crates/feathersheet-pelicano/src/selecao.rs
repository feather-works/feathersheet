//! Estado de seleção da grade: intervalo, âncora, célula ativa e tipo de eixo.
//! Nenhum widget nem avaliação de fórmula mora aqui.
use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Celula {
    pub linha: u32,
    pub coluna: u32,
}
impl Celula {
    pub fn nova(linha: u32, coluna: u32) -> Self {
        Self { linha: linha.min(MAX_ROWS - 1), coluna: coluna.min(MAX_COLS - 1) }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Alvo {
    Celula(Celula),
    Linha(u32),
    Coluna(u32),
    Tudo,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Eixo { #[default] Celulas, Linhas, Colunas, Tudo }

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Selecao {
    pub ancora: Celula,
    pub ativa: Celula,
    pub eixo: Eixo,
}
impl Default for Selecao {
    fn default() -> Self {
        Self { ancora: Celula::nova(0, 0), ativa: Celula::nova(0, 0), eixo: Eixo::Celulas }
    }
}
impl Selecao {
    pub fn unico(&mut self, pos: Celula) {
        self.ancora = pos;
        self.ativa = pos;
        self.eixo = Eixo::Celulas;
    }
    /// Shift mantém a âncora. Selecionar cabeçalho expande a linha ou coluna inteira.
    pub fn selecionar(&mut self, alvo: Alvo, estender: bool) {
        let (p, eixo) = match alvo {
            Alvo::Celula(p) => (p, Eixo::Celulas),
            Alvo::Linha(r) => (Celula::nova(r, self.ativa.coluna), Eixo::Linhas),
            Alvo::Coluna(c) => (Celula::nova(self.ativa.linha, c), Eixo::Colunas),
            Alvo::Tudo => (Celula::nova(MAX_ROWS - 1, MAX_COLS - 1), Eixo::Tudo),
        };
        if !estender || matches!(alvo, Alvo::Tudo) {
            self.ancora = match alvo {
                Alvo::Tudo => Celula::nova(0, 0),
                _ => p,
            };
        }
        self.ativa = p;
        self.eixo = if estender && self.eixo == eixo { self.eixo } else { eixo };
    }
    pub fn estender(&mut self, alvo: Alvo) {
        self.selecionar(alvo, true);
    }
    pub fn mover(&mut self, dr: i64, dc: i64, estender: bool) {
        let p = Celula::nova(
            (self.ativa.linha as i64 + dr).clamp(0, (MAX_ROWS - 1) as i64) as u32,
            (self.ativa.coluna as i64 + dc).clamp(0, (MAX_COLS - 1) as i64) as u32,
        );
        if estender {
            self.ativa = p;
            self.eixo = Eixo::Celulas;
        } else { self.unico(p); }
    }
    pub fn limites(&self) -> (u32, u32, u32, u32) {
        let r0 = self.ancora.linha.min(self.ativa.linha);
        let r1 = self.ancora.linha.max(self.ativa.linha);
        let c0 = self.ancora.coluna.min(self.ativa.coluna);
        let c1 = self.ancora.coluna.max(self.ativa.coluna);
        match self.eixo {
            Eixo::Celulas => (r0, c0, r1, c1),
            Eixo::Linhas => (r0, 0, r1, MAX_COLS - 1),
            Eixo::Colunas => (0, c0, MAX_ROWS - 1, c1),
            Eixo::Tudo => (0, 0, MAX_ROWS - 1, MAX_COLS - 1),
        }
    }
    pub fn contem(&self, r: u32, c: u32) -> bool {
        let (r0, c0, r1, c1) = self.limites();
        r >= r0 && r <= r1 && c >= c0 && c <= c1
    }
    pub fn tamanho(&self) -> (u64, u64) {
        let (r0, c0, r1, c1) = self.limites();
        ((r1-r0) as u64 + 1, (c1-c0) as u64 + 1)
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn selecao_por_shift_e_arrasto_preserva_ancora() {
        let mut s = Selecao::default();
        s.selecionar(Alvo::Celula(Celula::nova(2, 3)), false);
        s.estender(Alvo::Celula(Celula::nova(9, 7)));
        assert_eq!(s.limites(), (2,3,9,7));
        assert_eq!(s.ancora, Celula::nova(2,3));
        assert_eq!(s.ativa, Celula::nova(9,7));
        s.estender(Alvo::Celula(Celula::nova(1, 1)));
        assert_eq!(s.limites(), (1,1,2,3));
    }
    #[test]
    fn cabecalhos_expandem_eixos_sem_alocar_celulas() {
        let mut s=Selecao::default();
        s.selecionar(Alvo::Linha(8),false);
        s.estender(Alvo::Linha(11));
        assert_eq!(s.limites(),(8,0,11,MAX_COLS-1));
        s.selecionar(Alvo::Coluna(5),false);
        s.estender(Alvo::Coluna(7));
        assert_eq!(s.limites(),(0,5,MAX_ROWS-1,7));
        s.selecionar(Alvo::Tudo,false);
        assert_eq!(s.limites(),(0,0,MAX_ROWS-1,MAX_COLS-1));
    }
    #[test]
    fn setas_e_shift_respeitam_limites() {
        let mut s=Selecao::default();
        s.mover(-100,-10,false);
        assert_eq!(s.ativa,Celula::nova(0,0));
        s.mover(2,1,false);
        s.mover(3,3,true);
        assert_eq!(s.limites(),(2,1,5,4));
        s.mover(-100,0,true);
        assert_eq!(s.limites(),(0,1,2,4));
    }
}
