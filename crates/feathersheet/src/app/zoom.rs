//! Degraus de zoom migrados do TypeScript, para o rodapé e Ctrl+roda.
//! Limites podem ser alterados pelo usuário em Configurações → Aparência.
pub(super) const PASSOS: [u16; 11] = [25, 50, 75, 90, 100, 125, 150, 200, 250, 300, 400];

/// Próximo nível dentro da faixa. Extremos personalizados também são atingíveis.
pub(super) fn proximo(atual: u16, direcao: i32, minimo: u16, maximo: u16) -> Option<u16> {
    let minimo = minimo.clamp(25, 400);
    let maximo = maximo.clamp(minimo, 400);
    let atual = atual.clamp(minimo, maximo);
    if direcao > 0 {
        PASSOS.iter().copied().find(|&n| n > atual && n <= maximo)
            .or_else(|| (atual < maximo).then_some(maximo))
    } else if direcao < 0 {
        PASSOS.iter().rev().copied().find(|&n| n < atual && n >= minimo)
            .or_else(|| (atual > minimo).then_some(minimo))
    } else {
        None
    }
}
#[cfg(test)]
mod testes {
    use super::*;
    #[test]
    fn passos_legados_e_extremos() {
        assert_eq!(proximo(100, 1, 50, 200), Some(125));
        assert_eq!(proximo(100, -1, 50, 200), Some(90));
        assert_eq!(proximo(50, -1, 50, 200), None);
        assert_eq!(proximo(200, 1, 50, 200), None);
        assert_eq!(proximo(50, -1, 25, 400), Some(25));
        assert_eq!(proximo(200, 1, 25, 400), Some(250));
        assert_eq!(proximo(400, 1, 25, 400), None);
        assert_eq!(proximo(150, 1, 60, 180), Some(180));
        assert_eq!(proximo(90, -1, 80, 150), Some(80));
        assert_eq!(proximo(100, 0, 50, 200), None);
        assert_eq!(proximo(100, 1, 100, 100), None);
    }
}
