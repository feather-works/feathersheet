//! Política de backup de `src/io/backup-policy.ts`, migrada para Rust.
//!
//! Protege a interface de cópias frequentes muito caras. A gravação explícita
//! do usuário não é bloqueada por esta política.
pub const MAX_CELULAS_COPIA: u64 = 300_000;
pub const INTERVALO_MINIMO_MS: u64 = 800;
pub const INTERVALO_MAXIMO_MS: u64 = 30_000;
/// Acima disto não há cópia automática (recuperação nem salvamento
/// automático). Medido em 16/10/2026 com um .parquet de 633 milhões de
/// células: a fotografia trava a janela 3,3 s e dobra a memória (+4,4 GB), e
/// a gravação leva 60 s com pico de 12 GB — uma cópia sozinha arrisca
/// derrubar a máquina. Nessa pasta, só o Ctrl+S da pessoa grava.
/// Baixado para 1 milhão em 01/11/2026 (#136): a fotografia roda na thread da janela, e num
/// `.xlsx` de 3,2 milhões de células com 2,3 milhões de fórmulas ela travava a janela 2,8 s a
/// cada cópia e deixava o processo em 4,8 GB.
// shortcut: o limite protege a janela porque a fotografia (`da_workbook`) é feita nela; tirar a
// fotografia da thread da janela e voltar a proteger as pastas grandes.
pub const LIMITE_CELULAS_COPIA_AUTOMATICA: u64 = 1_000_000;

/// A pasta é grande demais para cópia automática (ver o limite acima).
#[inline]
pub fn grande_demais_para_copia_automatica(celulas: u64) -> bool {
    celulas > LIMITE_CELULAS_COPIA_AUTOMATICA
}

/// Mesmo critério do TypeScript: não fazer cópia rápida durante importação
/// em lote ou para mais de 300 mil células.
#[inline]
pub fn copia_vale_a_pena(celulas: u64, importacao_em_lote: bool) -> bool {
    !importacao_em_lote && celulas <= MAX_CELULAS_COPIA
}

/// Mantém a UI gastando no máximo ~10% do tempo com snapshots rápidos.
/// Saturação é importante para cronômetros e arquivos excepcionalmente grandes.
#[inline]
pub fn intervalo_da_copia(custo_ms: u64) -> u64 {
    custo_ms.saturating_mul(10).clamp(INTERVALO_MINIMO_MS, INTERVALO_MAXIMO_MS)
}

/// Intervalo efetivo do autosave FSHZ nativo.
///
/// Ao contrário do backup rápido descartável do navegador, salvar FSHZ em
/// disco é recuperação durável: NUNCA é desligado silenciosamente para uma
/// pasta grande. Em vez disso é limitado à cadência de 30 segundos, mesmo
/// quando o usuário configurou intervalos mais curtos. Salvar manualmente não
/// passa por este cálculo.
pub fn intervalo_autosave(
    configurado_ms: u64,
    custo_ultima_gravacao_ms: u64,
    celulas: u64,
    importacao_em_lote: bool,
) -> u64 {
    let minimo = if copia_vale_a_pena(celulas, importacao_em_lote) {
        intervalo_da_copia(custo_ultima_gravacao_ms)
    } else {
        // Pasta grande: no mínimo 30 s e, se gravar demora, dez vezes o
        // tempo da última gravação (gravar 25 s → a cada ~4 min), para o app
        // nunca passar o tempo todo gravando.
        INTERVALO_MAXIMO_MS.max(custo_ultima_gravacao_ms.saturating_mul(10))
    };
    configurado_ms.max(minimo)
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn limites_e_paridade_com_o_typescript() {
        assert!(copia_vale_a_pena(300_000,false));
        assert!(!copia_vale_a_pena(300_001,false));
        assert!(!copia_vale_a_pena(1,true));
        for (custo, espera) in [
            (0,800), (20,800), (80,800), (150,1_500),
            (301,3_010), (8_000,30_000),
            (u64::MAX,30_000),
        ] {
            assert_eq!(intervalo_da_copia(custo),espera);
        }
    }

    #[test]
    fn autosave_nao_desaparece_para_pastas_grandes() {
        assert_eq!(intervalo_autosave(2_000,0,100,false),2_000);
        assert_eq!(intervalo_autosave(2_000,800,100,false),8_000);
        assert_eq!(intervalo_autosave(2_000,0,300_001,false),30_000);
        assert_eq!(intervalo_autosave(2_000,0,200,true),30_000);
        assert_eq!(intervalo_autosave(60_000,0,300_001,false),60_000);
        // Pasta grande que leva 25 s para gravar: a cada ~4 min, não a cada 30 s.
        assert_eq!(intervalo_autosave(15_000,25_000,13_000_000,false),250_000);
    }
}
