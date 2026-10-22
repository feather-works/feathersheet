//! Máscara de células mescladas visíveis no Canvas.
//!
//! O desenho normal (uma célula por vez) NÃO deve escrever texto em uma
//! mesclagem: a segunda passagem pinta o texto da âncora uma única vez
//! com largura/altura da região. Só a camada visual é afetada; o Sheet
//! continua guardando os valores originais, que reaparecem ao desmesclar.
use feathersheet_core::celulas::refs::Rect;
use std::collections::HashSet;

/// Constrói a máscara somente para células existentes no viewport.
/// Não itera todos os índices de uma mescla de milhões de linhas.
pub(super) fn mascarar(
    regioes: &[Rect],
    linhas_visiveis: &[(u32, f32, f32)],
    colunas_visiveis: &[(u32, f32, f32)],
) -> HashSet<(u32, u32)> {
    let mut ocultar = HashSet::new();
    for area in regioes {
        for &(r, _, _) in linhas_visiveis {
            if r < area.r0 || r > area.r1 { continue; }
            for &(c, _, _) in colunas_visiveis {
                if c >= area.c0 && c <= area.c1 {
                    ocultar.insert((r, c));
                }
            }
        }
    }
    ocultar
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn duas_linhas_mescladas_nao_recebem_texto_individual() {
        let linhas = [(0, 30.0, 28.0), (1, 58.0, 28.0), (2, 86.0, 28.0)];
        let colunas = [(0, 90.0, 122.0), (1, 212.0, 122.0)];
        let ocultar = mascarar(&[Rect::new(0, 0, 1, 0)], &linhas, &colunas);
        // A âncora também deve pular o desenho individual: o Canvas a
        // desenhará somente na passagem unificada da mesclagem.
        assert!(ocultar.contains(&(0, 0)));
        assert!(ocultar.contains(&(1, 0)));
        assert!(!ocultar.contains(&(2, 0)));
        assert!(!ocultar.contains(&(0, 1)));
        assert_eq!(ocultar.len(), 2);
    }

    #[test]
    fn mescla_extensa_olha_apenas_linhas_visiveis() {
        let linhas = [(0, 30.0, 28.0), (150_000, 58.0, 28.0)];
        let colunas = [(2, 90.0, 122.0), (5, 212.0, 122.0)];
        let ocultar = mascarar(
            &[Rect::new(0, 2, 999_999, 5)], &linhas, &colunas,
        );
        assert_eq!(ocultar.len(), 4);
        assert!(ocultar.contains(&(150_000, 5)));
    }

    #[test]
    fn sem_mesclagem_todas_as_celulas_continuam_visiveis() {
        let linhas = [(1, 30.0, 28.0), (2, 58.0, 28.0)];
        let colunas = [(0, 90.0, 122.0)];
        assert!(mascarar(&[], &linhas, &colunas).is_empty());
    }
}
