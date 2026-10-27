//! Configuração de impressão da aba: papel, orientação, escala, margens,
//! área de impressão e quebras de página.
//!
//! Só os tipos e as contas de área útil. Ainda não é lido nem gravado em
//! arquivo, e nenhuma aba usa estes tipos ainda.

use std::collections::BTreeSet;

/// Milímetro em pontos tipográficos (1 pt = 1/72 polegada).
const PT_POR_MM: f32 = 72.0 / 25.4;

/// Tamanho de folha suportado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Papel {
    #[default]
    A4,
    Carta,
    /// Ofício, também chamado Legal.
    Oficio,
    A3,
    A5,
    /// B4 e B5 na medida JIS, a do Excel.
    B4,
    B5,
    /// Executive (7,25 × 10,5 pol.).
    Executivo,
    /// Tabloid (11 × 17 pol.).
    Tabloide,
}

impl Papel {
    /// Tamanho `(largura, altura)` em pontos, sempre no retrato.
    pub fn tamanho_pt(&self) -> (f32, f32) {
        match self {
            Papel::A4 => (595.28, 841.89),
            Papel::Carta => (612.0, 792.0),
            Papel::Oficio => (612.0, 1008.0),
            Papel::A3 => (841.89, 1190.55),
            Papel::A5 => (419.53, 595.28),
            Papel::B4 => (257.0 * PT_POR_MM, 364.0 * PT_POR_MM),
            Papel::B5 => (182.0 * PT_POR_MM, 257.0 * PT_POR_MM),
            Papel::Executivo => (522.0, 756.0),
            Papel::Tabloide => (792.0, 1224.0),
        }
    }
}

/// Orientação da folha.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Orientacao {
    #[default]
    Retrato,
    Paisagem,
}

/// Como a aba é reduzida para caber na folha.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Escala {
    /// Porcentagem fixa, por exemplo 100 para tamanho natural.
    Porcentagem(u16),
    /// Dimensionar para Ajustar do Excel: no máximo `largura` páginas de largura e
    /// `altura` de altura; 0 = automático (sem limite nessa direção). Nunca amplia.
    Paginas { largura: u16, altura: u16 },
}

impl Default for Escala {
    fn default() -> Self {
        Escala::Porcentagem(100)
    }
}

/// Margens da folha, em milímetros.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Margens {
    pub esquerda_mm: f32,
    pub direita_mm: f32,
    pub topo_mm: f32,
    pub base_mm: f32,
}

impl Default for Margens {
    fn default() -> Self {
        Margens {
            esquerda_mm: 12.0,
            direita_mm: 12.0,
            topo_mm: 12.0,
            base_mm: 12.0,
        }
    }
}

/// Configuração de página da aba.
#[derive(Debug, Clone, PartialEq)]
pub struct ConfigPagina {
    pub papel: Papel,
    pub orientacao: Orientacao,
    pub escala: Escala,
    pub margens: Margens,
    /// Imprimir as linhas de grade da aba.
    pub linhas_de_grade: bool,
    /// Imprimir os cabeçalhos de linha e coluna (A, B, 1, 2...).
    pub cabecalhos: bool,
    /// Texto do cabeçalho da página.
    pub cabecalho: String,
    /// Texto do rodapé da página.
    pub rodape: String,
}

impl Default for ConfigPagina {
    fn default() -> Self {
        ConfigPagina {
            papel: Papel::default(),
            orientacao: Orientacao::default(),
            escala: Escala::default(),
            margens: Margens::default(),
            linhas_de_grade: false,
            cabecalhos: false,
            cabecalho: String::new(),
            rodape: String::new(),
        }
    }
}

impl ConfigPagina {
    /// Área útil `(largura, altura)` em pontos: o papel já girado pela
    /// orientação, menos as margens.
    pub fn area_util_pt(&self) -> (f32, f32) {
        let (larg, alt) = self.papel.tamanho_pt();
        let (larg, alt) = match self.orientacao {
            Orientacao::Retrato => (larg, alt),
            Orientacao::Paisagem => (alt, larg),
        };
        let m = &self.margens;
        let horizontal = (m.esquerda_mm + m.direita_mm) * PT_POR_MM;
        let vertical = (m.topo_mm + m.base_mm) * PT_POR_MM;
        (larg - horizontal, alt - vertical)
    }
}

/// Área retangular da aba a imprimir. Os índices são inclusivos.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AreaImpressao {
    pub linha0: u32,
    pub coluna0: u32,
    pub linha1: u32,
    pub coluna1: u32,
}

/// Configuração completa de impressão de uma aba.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct Impressao {
    pub config: ConfigPagina,
    /// `None` imprime a região usada da aba.
    pub area: Option<AreaImpressao>,
    /// Linhas em que começa uma nova página (quebra manual).
    pub quebras_linha: BTreeSet<u32>,
    /// Colunas em que começa uma nova página (quebra manual).
    pub quebras_coluna: BTreeSet<u32>,
    /// Mostrar as quebras de página na grade da tela.
    pub mostrar_quebras: bool,
}

#[cfg(test)]
mod testes {
    use super::*;

    fn perto(a: f32, b: f32) -> bool {
        (a - b).abs() < 0.01
    }

    #[test]
    fn valores_padrao() {
        let config = ConfigPagina::default();
        assert_eq!(config.papel, Papel::A4);
        assert_eq!(config.orientacao, Orientacao::Retrato);
        assert_eq!(config.escala, Escala::Porcentagem(100));
        assert_eq!(config.margens, Margens::default());
        assert!(perto(config.margens.esquerda_mm, 12.0));
        assert!(perto(config.margens.direita_mm, 12.0));
        assert!(perto(config.margens.topo_mm, 12.0));
        assert!(perto(config.margens.base_mm, 12.0));
        assert!(!config.linhas_de_grade);
        assert!(!config.cabecalhos);
        assert!(config.cabecalho.is_empty());
        assert!(config.rodape.is_empty());

        let impressao = Impressao::default();
        assert_eq!(impressao.config, ConfigPagina::default());
        assert!(impressao.area.is_none());
        assert!(impressao.quebras_linha.is_empty());
        assert!(impressao.quebras_coluna.is_empty());
        assert!(!impressao.mostrar_quebras);
    }

    #[test]
    fn tamanho_de_cada_papel() {
        assert_eq!(Papel::A4.tamanho_pt(), (595.28, 841.89));
        assert_eq!(Papel::Carta.tamanho_pt(), (612.0, 792.0));
        assert_eq!(Papel::Oficio.tamanho_pt(), (612.0, 1008.0));
        // A3 é o dobro do A4 deitado; A5, a metade.
        assert_eq!(Papel::A3.tamanho_pt().0, Papel::A4.tamanho_pt().1);
        assert_eq!(Papel::A5.tamanho_pt().1, Papel::A4.tamanho_pt().0);
        assert!((Papel::B5.tamanho_pt().0 - 515.91).abs() < 0.01);
    }

    #[test]
    fn area_util_a4_retrato_com_margens_padrao() {
        let config = ConfigPagina::default();
        let (larg, alt) = config.area_util_pt();
        let margem = 24.0 * PT_POR_MM;
        assert!(perto(larg, 595.28 - margem), "largura {larg}");
        assert!(perto(alt, 841.89 - margem), "altura {alt}");
    }

    #[test]
    fn area_util_a4_paisagem_com_margens_padrao() {
        let config = ConfigPagina {
            orientacao: Orientacao::Paisagem,
            ..ConfigPagina::default()
        };
        let (larg, alt) = config.area_util_pt();
        let margem = 24.0 * PT_POR_MM;
        assert!(perto(larg, 841.89 - margem), "largura {larg}");
        assert!(perto(alt, 595.28 - margem), "altura {alt}");
    }
}
