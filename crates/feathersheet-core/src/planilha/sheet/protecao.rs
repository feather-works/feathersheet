//! Proteger planilha do Excel (Página Inicial → Formatar ▾ → Proteger
//! planilha…): com a aba protegida, célula **bloqueada** não muda e só o que
//! as permissões liberam é aceito. Bloqueada é o padrão, como no Excel: só a
//! célula com `locked: false` no estilo (Formatar células → Proteção) fica
//! livre para digitar.
//!
//! O motor guarda a proteção e responde as perguntas; quem vigia é a
//! interface, que examina o que cada ação deixou no desfazer
//! ([`Sheet::examinar_protecao`]) e desfaz o que não podia. A senha vem
//! pronta (hash no formato do Excel, calculado fora do motor): aqui ela é
//! só guardada e devolvida ao arquivo.
use std::collections::BTreeSet;

use super::Sheet;
use crate::celulas::estilo_parcial::Campo;
use crate::celulas::refs::Rect;

/// O que o Excel deixa marcar em "Permitir que todos os usuários desta
/// planilha possam".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Permissao {
    SelecionarBloqueadas,
    SelecionarDesbloqueadas,
    FormatarCelulas,
    FormatarColunas,
    FormatarLinhas,
    InserirColunas,
    InserirLinhas,
    InserirLinks,
    ExcluirColunas,
    ExcluirLinhas,
    Classificar,
    AutoFiltro,
    TabelaDinamica,
    EditarObjetos,
    EditarCenarios,
}

impl Permissao {
    /// Todas, na ordem da janela do Excel: (permissão, texto, atributo do
    /// `<sheetProtection>` do `.xlsx`).
    pub const TODAS: [(Permissao, &'static str, &'static str); 15] = [
        (Permissao::SelecionarBloqueadas, "Selecionar células bloqueadas", "selectLockedCells"),
        (Permissao::SelecionarDesbloqueadas, "Selecionar células desbloqueadas", "selectUnlockedCells"),
        (Permissao::FormatarCelulas, "Formatar células", "formatCells"),
        (Permissao::FormatarColunas, "Formatar colunas", "formatColumns"),
        (Permissao::FormatarLinhas, "Formatar linhas", "formatRows"),
        (Permissao::InserirColunas, "Inserir colunas", "insertColumns"),
        (Permissao::InserirLinhas, "Inserir linhas", "insertRows"),
        (Permissao::InserirLinks, "Inserir hiperlinks", "insertHyperlinks"),
        (Permissao::ExcluirColunas, "Excluir colunas", "deleteColumns"),
        (Permissao::ExcluirLinhas, "Excluir linhas", "deleteRows"),
        (Permissao::Classificar, "Classificar", "sort"),
        (Permissao::AutoFiltro, "Usar AutoFiltro", "autoFilter"),
        (Permissao::TabelaDinamica, "Usar tabela dinâmica e gráfico dinâmico", "pivotTables"),
        (Permissao::EditarObjetos, "Editar objetos", "objects"),
        (Permissao::EditarCenarios, "Editar cenários", "scenarios"),
    ];

    pub fn chave(self) -> &'static str {
        Self::TODAS.iter().find(|(p, ..)| *p == self).map(|(_, _, a)| *a).expect("todas listadas")
    }

    pub fn da_chave(chave: &str) -> Option<Self> {
        Self::TODAS.iter().find(|(_, _, a)| *a == chave).map(|(p, ..)| *p)
    }

    /// O padrão da janela: só selecionar.
    pub fn padrao() -> BTreeSet<Permissao> {
        [Permissao::SelecionarBloqueadas, Permissao::SelecionarDesbloqueadas].into_iter().collect()
    }
}

/// Hash da senha no formato do `<sheetProtection>` do Excel (SHA-512 com
/// sal e giros), para o `.xlsx` abrir protegido no Excel com a mesma senha.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SenhaPlanilha {
    pub algoritmo: String,
    /// Base64.
    pub hash: String,
    /// Base64.
    pub sal: String,
    pub giros: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProtecaoPlanilha {
    pub senha: Option<SenhaPlanilha>,
    pub permitidas: BTreeSet<Permissao>,
}

impl ProtecaoPlanilha {
    pub fn permite(&self, p: Permissao) -> bool {
        self.permitidas.contains(&p)
    }
}

/// Por que a ação foi recusada. O texto é o que a interface mostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Recusa(pub String);

impl Recusa {
    pub fn celula_bloqueada() -> Self {
        Recusa(crate::tr!("A célula que você está tentando alterar está em uma planilha protegida. Para alterá-la, desproteja a planilha (menu Formatar, Proteger ou desproteger planilha).").into())
    }
    pub fn sem_permissao(p: Permissao) -> Self {
        let texto = Permissao::TODAS.iter().find(|(x, ..)| *x == p).map(|(_, t, _)| *t).unwrap_or("isso");
        Recusa(crate::trf!("A planilha está protegida e não permite \"{}\".", texto.to_lowercase()))
    }
    pub fn planilha_protegida() -> Self {
        Recusa(crate::tr!("A planilha está protegida: esta ação não está disponível. Desproteja a planilha para continuar.").into())
    }
}

/// Acima disto, perguntar célula a célula não compensa: a área é tratada
/// pelo que vale para ela inteira (estilo da aba, das colunas ou das linhas).
const AREA_CELULA_A_CELULA: u64 = 200_000;

fn trava(estilo: Option<&crate::celulas::estilo_parcial::EstiloParcial>, chave: &str) -> Option<bool> {
    match estilo?.get(chave) {
        Some(Campo::Logico(v)) => Some(*v),
        _ => None,
    }
}

impl Sheet {
    pub fn protegida(&self) -> bool {
        self.protecao.is_some()
    }

    /// Se a aba está protegida e não permite `p`.
    pub fn proibe(&self, p: Permissao) -> bool {
        self.protecao.as_ref().is_some_and(|x| !x.permite(p))
    }

    /// Bloqueada no estilo (o padrão é sim), protegida ou não.
    pub fn celula_marcada_bloqueada(&self, row: u32, col: u32) -> bool {
        trava(self.estilo_mesclado(row, col).as_ref(), "locked").unwrap_or(true)
    }

    /// A célula não pode mudar agora: aba protegida e célula bloqueada.
    pub fn celula_bloqueada(&self, row: u32, col: u32) -> bool {
        self.protegida() && self.celula_marcada_bloqueada(row, col)
    }

    /// Com a aba protegida, a fórmula desta célula não aparece na barra.
    pub fn formula_oculta(&self, row: u32, col: u32) -> bool {
        self.protegida() && trava(self.estilo_mesclado(row, col).as_ref(), "hidden").unwrap_or(false)
    }

    /// Alguma célula da área está bloqueada (com a aba protegida)?
    pub fn area_bloqueada(&self, area: &Rect) -> bool {
        if !self.protegida() { return false; }
        let a = area.normalized();
        let celulas = (a.r1 - a.r0 + 1) as u64 * (a.c1 - a.c0 + 1) as u64;
        if celulas <= AREA_CELULA_A_CELULA {
            return (a.r0..=a.r1).any(|r| (a.c0..=a.c1).any(|c| self.celula_marcada_bloqueada(r, c)));
        }
        // Área enorme: livre só se a aba inteira, ou cada coluna da área, foi
        // desbloqueada — e nenhuma célula com estilo próprio voltou a travar.
        let livre_por_camada = trava(self.sheet_style.as_ref(), "locked") == Some(false)
            || (a.c1 - a.c0 < 16_384 && (a.c0..=a.c1).all(|c| trava(self.col_styles.get(&c), "locked") == Some(false)));
        if !livre_por_camada { return true; }
        self.styles.alguma_no_retangulo(a.r0, a.c0, a.r1, a.c1, |r, c, _| self.celula_marcada_bloqueada(r, c))
    }

    /// Protege a aba (sem desfazer, como no Excel). Falso se já estava.
    pub fn proteger(&mut self, protecao: ProtecaoPlanilha) -> bool {
        if self.protegida() { return false; }
        self.protecao = Some(protecao);
        self.tocar();
        true
    }

    /// Tira a proteção. Quem pede a senha e a confere é a interface.
    pub fn desproteger(&mut self) -> bool {
        let tinha = self.protecao.take().is_some();
        if tinha { self.tocar(); }
        tinha
    }
}

#[cfg(test)]
mod testes {
    use super::*;
    use crate::celulas::estilo_parcial::EstiloParcial;

    fn livre() -> EstiloParcial {
        let mut e = EstiloParcial::new();
        e.insert("locked".into(), Campo::Logico(false));
        e
    }

    #[test]
    fn bloqueada_e_o_padrao_e_so_vale_protegida() {
        let mut s = Sheet::new("A");
        s.set_style(&Rect::cell(1, 1), &livre());
        assert!(!s.celula_bloqueada(0, 0), "sem proteção nada trava");
        s.proteger(ProtecaoPlanilha { senha: None, permitidas: Permissao::padrao() });
        assert!(s.celula_bloqueada(0, 0));
        assert!(!s.celula_bloqueada(1, 1));
        assert!(s.area_bloqueada(&Rect::new(0, 0, 1, 1)));
        assert!(!s.area_bloqueada(&Rect::cell(1, 1)));
        assert!(s.proibe(Permissao::FormatarCelulas));
        assert!(!s.proibe(Permissao::SelecionarBloqueadas));
        assert!(s.desproteger() && !s.celula_bloqueada(0, 0));
    }

    #[test]
    fn coluna_inteira_desbloqueada_e_area_enorme() {
        let mut s = Sheet::new("A");
        s.col_styles.insert(2, livre());
        s.proteger(ProtecaoPlanilha { senha: None, permitidas: BTreeSet::new() });
        assert!(!s.area_bloqueada(&Rect::new(0, 2, crate::celulas::refs::MAX_ROWS - 1, 2)));
        assert!(s.area_bloqueada(&Rect::new(0, 1, crate::celulas::refs::MAX_ROWS - 1, 2)));
        assert_eq!(Permissao::da_chave("sort"), Some(Permissao::Classificar));
        assert_eq!(Permissao::InserirLinhas.chave(), "insertRows");
    }

    #[test]
    fn examinar_recusa_e_desfazer_ate_apaga_a_acao() {
        let mut s = Sheet::new("A");
        s.set_cell(0, 0, "fixo");
        s.set_style(&Rect::cell(1, 0), &livre());
        s.clear_history();
        s.proteger(ProtecaoPlanilha { senha: None, permitidas: Permissao::padrao() });
        // Célula livre: pode.
        let marca = s.profundidade_do_historico();
        s.set_cell(1, 0, "digitado");
        assert_eq!(s.examinar_protecao(marca), None);
        // Bloqueada: recusa, e o desfazer devolve o texto sem deixar refazer.
        let marca = s.profundidade_do_historico();
        s.set_cell(0, 0, "mudou");
        assert_eq!(s.examinar_protecao(marca), Some(Recusa::celula_bloqueada()));
        s.desfazer_ate(marca);
        assert_eq!(s.get_raw(0, 0), "fixo");
        assert!(!s.redo(), "a ação recusada não fica no refazer");
        // Formatar a célula livre pede a permissão.
        let marca = s.profundidade_do_historico();
        let mut negrito = EstiloParcial::new();
        negrito.insert("bold".into(), Campo::Logico(true));
        s.set_style(&Rect::cell(1, 0), &negrito);
        assert_eq!(s.examinar_protecao(marca), Some(Recusa::sem_permissao(Permissao::FormatarCelulas)));
        s.desfazer_ate(marca);
        s.protecao.as_mut().unwrap().permitidas.insert(Permissao::FormatarCelulas);
        let marca = s.profundidade_do_historico();
        s.set_style(&Rect::cell(1, 0), &negrito);
        assert_eq!(s.examinar_protecao(marca), None);
        // Largura de coluna pede Formatar colunas.
        let marca = s.profundidade_do_historico();
        s.definir_dimensao(true, 3, 200);
        assert_eq!(s.examinar_protecao(marca), Some(Recusa::sem_permissao(Permissao::FormatarColunas)));
    }
}
