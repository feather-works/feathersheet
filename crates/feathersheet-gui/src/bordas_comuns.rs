//! APP-010: bordas do armazenamento colunar Sheet para os tipos de Tables.
//!
//! É intencional que nenhum `Tabela` seja materializado: o Canvas visita
//! somente as células do viewport. `feathersheet-core::Style` e os arquivos
//! .fshz permanecem sendo a fonte de verdade; este módulo só projeta traços.

use feathersheet_core::celulas::style::Borders;
use feather_tables::{BordaVisual, Traco};

/// A UI antiga pintava bordas sólidas com 1,5 px lógicos, independente do zoom.
pub const ESPESSURA_LEGADA: f32 = 1.5;

/// Geometria/estilo neutros, em pixels lógicos da cena Canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SegmentoBorda {
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
    pub cor: [u8; 4],
}

fn cor_borda(cor: Option<&str>, fallback: [u8; 4]) -> [u8; 4] {
    cor.filter(|s| feather_ui::seletor_cor::hex_valido(s))
        .map(feather_ui::seletor_cor::rgba_de)
        .unwrap_or(fallback)
}

/// Converte os quatro booleanos legados no mesmo contrato de borda do
/// FeatherWorks Tables: ordem cima/direita/baixo/esquerda. Sem alocação,
/// cópia de células, alteração de estilos ou interpretação de fórmulas.
pub fn adaptar_bordas(bordas: Option<&Borders>, fallback: [u8; 4]) -> [BordaVisual; 4] {
    let Some(bordas) = bordas else {
        return [BordaVisual::default(); 4];
    };
    let cor = cor_borda(bordas.color.as_deref(), fallback);
    let lado = |visivel| BordaVisual {
        estilo: if visivel { Traco::Solido } else { Traco::Nenhum },
        espessura: if visivel { ESPESSURA_LEGADA } else { 0.0 },
        cor,
    };
    [
        lado(bordas.top),
        lado(bordas.right),
        lado(bordas.bottom),
        lado(bordas.left),
    ]
}

/// Gera no máximo 4 segmentos para o Canvas, sem criar uma Tabela
/// proporcional às 100 milhões de linhas. As bordas invisíveis são None.
/// Limita a espessura ao tamanho da célula e rejeita geometria inválida,
/// preservando o desenho existente quando as dimensões são normais.
pub fn segmentos(
    bordas: [BordaVisual; 4], x: f32, y: f32, largura: f32, altura: f32,
) -> [Option<SegmentoBorda>; 4] {
    let mut saida = [None; 4];
    if ![x, y, largura, altura].iter().all(|v| v.is_finite())
        || largura <= 0.0 || altura <= 0.0
        || !(x + largura).is_finite() || !(y + altura).is_finite()
    {
        return saida;
    }
    for (indice, borda) in bordas.iter().enumerate() {
        if borda.estilo == Traco::Nenhum
            || !borda.espessura.is_finite() || borda.espessura <= 0.0
        {
            continue;
        }
        // O Sheet original só tem borda sólida. Outras variantes do
        // contrato comum exigirão um desenho de dash explícito.
        if borda.estilo != Traco::Solido { continue; }
        let esp = borda.espessura.min(largura).min(altura);
        let (sx, sy, sw, sh) = match indice {
            0 => (x, y, largura, esp),
            1 => (x + largura - esp, y, esp, altura),
            2 => (x, y + altura - esp, largura, esp),
            3 => (x, y, esp, altura),
            _ => unreachable!(),
        };
        saida[indice] = Some(SegmentoBorda {
            x: sx, y: sy, largura: sw, altura: sh, cor: borda.cor,
        });
    }
    saida
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn mapeia_quatro_lados_na_ordem_compartilhada() {
        let b = Borders {
            top: true, right: false, bottom: true, left: false,
            color: Some("#ff804020".into()),
        };
        let comum = adaptar_bordas(Some(&b), [0, 0, 0, 255]);
        assert_eq!(comum[0].estilo, Traco::Solido);
        assert_eq!(comum[1].estilo, Traco::Nenhum);
        assert_eq!(comum[2].estilo, Traco::Solido);
        assert_eq!(comum[3].estilo, Traco::Nenhum);
        assert_eq!(comum[0].espessura, 1.5);
        assert_eq!(comum[0].cor, [255, 128, 64, 32]);
        assert_eq!(segmentos(comum, 10.0, 20.0, 100.0, 28.0)[0].unwrap(),
            SegmentoBorda { x: 10.0, y: 20.0,
                largura: 100.0, altura: 1.5, cor: [255, 128, 64, 32] });
        assert_eq!(segmentos(comum, 10.0, 20.0, 100.0, 28.0)[1], None);
        assert_eq!(segmentos(comum, 10.0, 20.0, 100.0, 28.0)[2].unwrap().y, 46.5);
    }

    #[test]
    fn falta_de_bordas_nao_cria_grade_densa() {
        let vazio = adaptar_bordas(None, [1, 2, 3, 255]);
        assert_eq!(vazio, [BordaVisual::default(); 4]);
        assert_eq!(segmentos(vazio, 0.0, 0.0, 122.0, 28.0), [None; 4]);
        let b = Borders::default();
        assert_eq!(segmentos(adaptar_bordas(Some(&b), [1, 2, 3, 255]),
            0.0, 0.0, 122.0, 28.0), [None; 4]);
    }

    #[test]
    fn cor_invalida_conserva_fallback_visual() {
        for cor in [None, Some("invalida".into()), Some("#zz0033".into())] {
            let b = Borders { top: true, color: cor, ..Default::default() };
            assert_eq!(adaptar_bordas(Some(&b), [70, 80, 90, 255])[0].cor,
                [70, 80, 90, 255]);
        }
    }

    #[test]
    fn segmentos_direita_e_esquerda_respeitam_dimensoes() {
        let b = Borders { right: true, left: true, ..Default::default() };
        let segmentos = segmentos(adaptar_bordas(Some(&b), [5, 6, 7, 255]),
            45.0, 30.0, 122.0, 28.0);
        assert_eq!(segmentos[1].unwrap(),
            SegmentoBorda { x: 165.5, y: 30.0, largura: 1.5,
                altura: 28.0, cor: [5, 6, 7, 255] });
        assert_eq!(segmentos[3].unwrap().x, 45.0);
    }

    #[test]
    fn celula_estreita_e_celula_oculta_nao_geram_overflow() {
        let b = Borders { top: true, bottom: true, right: true,
            left: true, ..Default::default() };
        let comum = adaptar_bordas(Some(&b), [0, 0, 0, 255]);
        assert_eq!(segmentos(comum, 0.0, 0.0, 0.0, 28.0), [None; 4]);
        let estreita = segmentos(comum, 0.0, 0.0, 0.5, 0.4);
        assert!(estreita.iter().flatten().all(|p|
            p.largura <= 0.5 && p.altura <= 0.4 && p.x >= 0.0 && p.y >= 0.0));
        assert_eq!(segmentos(comum, f32::NAN, 0.0, 4.0, 5.0), [None; 4]);
    }

    #[test]
    fn sem_estado_mutavel_ou_copia_de_workbook() {
        let b = Borders { top: true, color: Some("#123456".into()),
            ..Default::default() };
        let antes = b.clone();
        let a = adaptar_bordas(Some(&b), [0, 0, 0, 255]);
        let c = adaptar_bordas(Some(&b), [0, 0, 0, 255]);
        assert_eq!(a, c);
        assert_eq!(b, antes);
    }
}
