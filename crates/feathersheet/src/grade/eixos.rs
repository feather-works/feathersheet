//! Índice esparso de larguras e alturas da grade, migrado de ui/grade/axis.ts.
//! O custo de posicionar e saltar milhões de índices é logarítmico. Nenhuma
//! dimensão padrão é materializada; só existem entradas para exceções.
use feathersheet_core::planilha::workbook::Workbook;
use std::collections::HashMap;

/// Prefixos das diferenças para a dimensão padrão, em ordem por índice.
#[derive(Debug)]
pub(super) struct Eixo {
    quantidade: u32,
    padrao: f32,
    excecoes: Vec<(u32, f32)>,
    prefixos: Vec<f64>,
    /// Intervalos [início, fim) de linhas/colunas de tamanho zero.
    ocultos: Vec<(u32, u32)>,
    /// Daqui até o fim tudo está oculto (`quantidade` quando nada está).
    limite: u32,
}

impl Eixo {
    /// `ocultas_desde`: tudo a partir desse índice está oculto (ocultar da
    /// linha 30 até a última sem guardar uma altura por linha).
    pub(super) fn novo(quantidade: u32, padrao: f32, dimensoes: &HashMap<u32, u32>, ocultas_desde: Option<u32>) -> Self {
        let limite = ocultas_desde.map_or(quantidade, |n| n.min(quantidade));
        let mut excecoes: Vec<_> = dimensoes.iter()
            .filter(|(i, _)| **i < limite)
            .map(|(&i, &tamanho)| (i, tamanho as f32))
            .collect();
        excecoes.sort_unstable_by_key(|(i, _)| *i);

        let mut prefixos = Vec::with_capacity(excecoes.len() + 1);
        prefixos.push(0.0);
        let mut ocultos = Vec::new();
        for &(i, tamanho) in &excecoes {
            prefixos.push(prefixos.last().copied().unwrap_or(0.0)
                + f64::from(tamanho) - f64::from(padrao));
            if tamanho == 0.0 {
                match ocultos.last_mut() {
                    Some((_, fim)) if *fim == i => *fim = i + 1,
                    _ => ocultos.push((i, i + 1)),
                }
            }
        }
        if limite < quantidade {
            match ocultos.last_mut() {
                Some((_, fim)) if *fim == limite => *fim = quantidade,
                _ => ocultos.push((limite, quantidade)),
            }
        }
        Self { quantidade, padrao, excecoes, prefixos, ocultos, limite }
    }

    pub(super) fn tamanho(&self, indice: u32) -> f32 {
        if indice >= self.limite { return 0.0; }
        self.excecoes.binary_search_by_key(&indice, |(i, _)| *i)
            .ok().map(|pos| self.excecoes[pos].1).unwrap_or(self.padrao)
    }

    /// Posição absoluta do início do índice. O limite é inclusivo para
    /// permitir calcular a extensão total sem percorrer o eixo.
    #[allow(dead_code)]
    pub(super) fn inicio(&self, indice: u32) -> f64 {
        // Depois do limite tudo tem tamanho zero: a posição para de crescer.
        let indice = indice.min(self.quantidade).min(self.limite);
        let antes = self.excecoes.partition_point(|(i, _)| *i < indice);
        f64::from(indice) * f64::from(self.padrao) + self.prefixos[antes]
    }

    #[allow(dead_code)]
    pub(super) fn total(&self) -> f64 {
        self.inicio(self.quantidade)
    }

    /// Equivalente a indexAt do TS; útil para hit-test por posição absoluta.
    #[allow(dead_code)]
    pub(super) fn indice_em(&self, px: f64) -> Option<u32> {
        if self.quantidade == 0 { return None; }
        if px <= 0.0 { return Some(0); }
        if px >= self.total() { return Some(self.quantidade - 1); }
        let (mut lo, mut hi) = (0u32, self.quantidade - 1);
        while lo < hi {
            let meio = lo + (hi - lo).div_ceil(2);
            if self.inicio(meio) <= px { lo = meio; }
            else { hi = meio - 1; }
        }
        Some(lo)
    }

    /// Primeira linha que permite desenhar a cauda inteira do eixo
    /// (até a última linha) na altura fornecida, sem cortar a última.
    /// 
    /// Equivale ao laço reverso do Canvas anterior, mas a posição absoluta
    /// de cada linha vem dos prefixos esparsos de `axis.ts`. A busca não
    /// percorre milhões de linhas ocultas: O(log N * log exceções).
    ///
    /// Para uma altura menor que a última linha, mantém a última linha
    /// acessível mesmo sem caber inteira, como fazia a grade original.
    pub(super) fn inicio_da_cauda(&self, disponivel: f64) -> u32 {
        if self.quantidade <= 1 { return 0; }
        if !disponivel.is_finite() || disponivel <= 0.0 {
            return self.quantidade - 1;
        }
        let total = self.total();
        if disponivel > total { return 0; }
        let alvo = (total - disponivel).max(0.0);
        // Procura o PRIMEIRO prefixo estritamente maior que o alvo.
        // Se há igualdade exata, o laço antigo para ao incluir a linha
        // que esgota a altura; zeros adjacentes NÃO entram depois disso.
        // Em caso contrário, escolhe a primeira linha cuja cauda cabe.
        let (mut esquerda, mut direita) = (0u32, self.quantidade);
        while esquerda < direita {
            let meio = esquerda + (direita - esquerda) / 2;
            if self.inicio(meio) <= alvo {
                esquerda = meio + 1;
            } else {
                direita = meio;
            }
        }
        if esquerda > 0 && self.inicio(esquerda - 1) == alvo {
            esquerda - 1
        } else {
            esquerda.min(self.quantidade - 1)
        }
    }

    /// Avança por COLUNAS VISÍVEIS (não por índices lógicos), saltando
    /// intervalos ocultos e respeitando as colunas congeladas. Usa somente
    /// os prefixos de dimensões já cacheados no Canvas.
    ///
    /// O custo depende dos passos pedidos (normalmente 2 ou 3 pela roda),
    /// não da quantidade de colunas ocultas entre duas colunas visíveis.
    pub(super) fn deslocar_visiveis(&self, origem: u32, minimo: u32, passos: i32) -> u32 {
        if self.quantidade == 0 { return 0; }
        let minimo = minimo.min(self.quantidade - 1);
        let Some(primeiro) = self.proximo_visivel(minimo) else { return minimo; };
        let ultimo = self.anterior_visivel(self.quantidade - 1).unwrap_or(primeiro);
        if ultimo < primeiro { return minimo; }

        let normalizada = origem.clamp(primeiro, ultimo);
        let mut atual = if self.tamanho(normalizada) > 0.0 {
            normalizada
        } else {
            self.proximo_visivel(normalizada)
                .filter(|&indice| indice <= ultimo)
                .unwrap_or_else(|| self.anterior_visivel(normalizada).unwrap_or(primeiro))
        };
        if passos == 0 { return atual; }
        // O limite do eixo evita até mesmo um loop gigantesco para passos
        // artificiais enviados por comandos externos.
        for _ in 0..passos.unsigned_abs().min(self.quantidade) {
            let seguinte = if passos > 0 {
                self.proximo_visivel(atual.saturating_add(1))
                    .filter(|&indice| indice <= ultimo)
            } else {
                atual.checked_sub(1).and_then(|i| self.anterior_visivel(i))
                    .filter(|&indice| indice >= primeiro)
            };
            let Some(seguinte) = seguinte else { break; };
            atual = seguinte;
        }
        atual
    }

    /// Último índice visível até a posição informada (inclusive).
    /// Complemento reverso de `proximo_visivel` para a roda do mouse:
    /// não atravessa uma faixa oculta de 100 mil linhas em passos unitários.
    pub(super) fn anterior_visivel(&self, indice: u32) -> Option<u32> {
        if self.quantidade == 0 { return None; }
        let indice = indice.min(self.quantidade - 1);
        let anterior = self.ocultos.partition_point(|(inicio, _)| *inicio <= indice);
        if anterior > 0 && indice < self.ocultos[anterior - 1].1 {
            self.ocultos[anterior - 1].0.checked_sub(1)
        } else {
            Some(indice)
        }
    }

    /// Salta direto para depois da sequência de índices ocultos.
    /// O(ln exceções), mesmo que um milhão de linhas esteja oculto.
    pub(super) fn proximo_visivel(&self, indice: u32) -> Option<u32> {
        if indice >= self.quantidade { return None; }
        let anterior = self.ocultos.partition_point(|(ini, _)| *ini <= indice);
        let proximo = if anterior > 0 && indice < self.ocultos[anterior - 1].1 {
            self.ocultos[anterior - 1].1
        } else { indice };
        (proximo < self.quantidade).then_some(proximo)
    }
}

struct Cache {
    id: u32,
    versao: u32,
    linhas_count: usize,
    colunas_count: usize,
    desde: (Option<u32>, Option<u32>),
    linhas: Eixo,
    colunas: Eixo,
}

/// O cache é só de interface: não grava no FSHZ, não duplica células e não
/// interfere no histórico. A aba e sua versão invalidam os prefixos.
#[derive(Default)]
pub(crate) struct EstadoEixos {
    cache: Option<Cache>,
    /// Tamanho do Canvas da grade no último desenho (pixels de tela).
    pub(crate) tamanho_canvas: Option<pelicano::Size>,
}

impl EstadoEixos {
    pub(crate) fn limpar(&mut self) { self.cache = None; }

    /// Após ocultar a dimensão selecionada, encontra a próxima visível.
    /// Não percorre índices intermediários: usa intervalos ocultos do eixo
    /// cacheado também utilizado pelo Canvas e pela rolagem.
    pub(crate) fn visivel_apos_ocultar(
        &mut self, workbook: &Workbook, indice_aba: usize,
        coluna: bool, inicio: u32, fim: u32,
    ) -> Option<u32> {
        use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};
        let (linhas, colunas) = self.para(
            workbook, indice_aba, MAX_ROWS, MAX_COLS,
            super::ALTURA_PADRAO, super::LARGURA_PADRAO,
        )?;
        let eixo = if coluna { colunas } else { linhas };
        eixo.proximo_visivel(fim.saturating_add(1))
            .or_else(|| inicio.checked_sub(1).and_then(|i| eixo.anterior_visivel(i)))
    }

    /// Movimento horizontal pela roda e comandos de rolagem da janela.
    /// Reaproveita o MESMO cache por versão da aba que desenha a grade.
    pub(crate) fn coluna_apos_rolar(
        &mut self, workbook: &Workbook, indice_aba: usize,
        origem: u32, congeladas: u32, passos: i32,
    ) -> u32 {
        use feathersheet_core::celulas::refs::{MAX_COLS, MAX_ROWS};
        let fallback = (i64::from(origem) + i64::from(passos))
            .clamp(i64::from(congeladas.min(MAX_COLS - 1)), i64::from(MAX_COLS - 1))
            as u32;
        let Some((_, colunas)) = self.para(
            workbook, indice_aba, MAX_ROWS, MAX_COLS,
            super::ALTURA_PADRAO, super::LARGURA_PADRAO,
        ) else { return fallback; };
        colunas.deslocar_visiveis(origem, congeladas, passos)
    }

    pub(super) fn para<'a>(
        &'a mut self, workbook: &Workbook, indice_aba: usize,
        linhas_total: u32, colunas_total: u32, altura_padrao: f32, largura_padrao: f32,
    ) -> Option<(&'a Eixo, &'a Eixo)> {
        let aba = workbook.aba(indice_aba)?;
        let atualizar = self.cache.as_ref().is_none_or(|c| {
            c.id != aba.id || c.versao != aba.versao
                || c.linhas_count != aba.sheet.row_heights.len()
                || c.colunas_count != aba.sheet.col_widths.len()
                || c.desde != (aba.sheet.linhas_ocultas_desde, aba.sheet.colunas_ocultas_desde)
        });
        if atualizar {
            self.cache = Some(Cache {
                id: aba.id,
                versao: aba.versao,
                linhas_count: aba.sheet.row_heights.len(),
                colunas_count: aba.sheet.col_widths.len(),
                desde: (aba.sheet.linhas_ocultas_desde, aba.sheet.colunas_ocultas_desde),
                linhas: Eixo::novo(linhas_total, altura_padrao, &aba.sheet.row_heights, aba.sheet.linhas_ocultas_desde),
                // A Largura padrão da aba (Formatar ▾) vence a da grade.
                colunas: Eixo::novo(colunas_total,
                    aba.sheet.largura_padrao.map_or(largura_padrao, |v| v as f32), &aba.sheet.col_widths,
                    aba.sheet.colunas_ocultas_desde),
            });
        }
        let cache = self.cache.as_ref()?;
        Some((&cache.linhas, &cache.colunas))
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn prefixos_e_hit_test_equivalem_ao_eixo_typescript() {
        let medidas = HashMap::from([(1, 0), (2, 0), (3, 16), (7, 35), (99, 1)]);
        let e = Eixo::novo(10, 28., &medidas, None);
        assert_eq!(e.inicio(1), 28.);
        assert_eq!(e.inicio(3), 28.);
        assert_eq!(e.inicio(4), 44.);
        assert_eq!(e.total(), 28. * 10. - 56. - 12. + 7.);
        assert_eq!(e.indice_em(29.), Some(3));
        assert_eq!(e.proximo_visivel(1), Some(3));
        assert_eq!(e.proximo_visivel(2), Some(3));
        assert_eq!(e.proximo_visivel(4), Some(4));
    }

    #[test]
    fn milhoes_de_indices_ocultos_nao_sao_percorridos_para_saltar() {
        let mut medidas = HashMap::new();
        for i in 0..100_000 { medidas.insert(i, 0); }
        let e = Eixo::novo(10_000_000, 28., &medidas, None);
        assert_eq!(e.proximo_visivel(0), Some(100_000));
        assert_eq!(e.proximo_visivel(75_000), Some(100_000));
        assert_eq!(e.inicio(100_000), 0.);
        assert_eq!(e.inicio(100_001), 28.);
        let todas = Eixo::novo(2, 28., &HashMap::from([(0, 0), (1, 0)]), None);
        assert_eq!(todas.proximo_visivel(0), None);
        assert_eq!(todas.proximo_visivel(1), None);
    }

    /// Referência: versão direta do laço anterior, em eixo pequeno,
    /// para validar equivalência sob alturas variadas/ocultas.
    fn inicio_reverso_anterior(eixo: &Eixo, mut restante: f32) -> u32 {
        if restante <= 0.0 { return eixo.quantidade.saturating_sub(1); }
        let mut inicio = eixo.quantidade;
        while inicio > 0 {
            let h = eixo.tamanho(inicio - 1);
            if h > restante && inicio < eixo.quantidade { break; }
            restante -= h;
            inicio -= 1;
            if restante <= 0.0 { break; }
        }
        inicio.min(eixo.quantidade - 1)
    }

    #[test]
    fn rolagem_horizontal_conta_colunas_visiveis_nao_indices_ocultos() {
        let eixo = Eixo::novo(12, 122., &HashMap::from([
            (2, 0), (3, 0), (8, 0),
        ]), None);
        assert_eq!(eixo.deslocar_visiveis(1, 0, 1), 4);
        assert_eq!(eixo.deslocar_visiveis(1, 0, 2), 5);
        assert_eq!(eixo.deslocar_visiveis(7, 0, -2), 5);
        assert_eq!(eixo.deslocar_visiveis(4, 0, -1), 1);
        assert_eq!(eixo.deslocar_visiveis(7, 4, -100), 4);
        assert_eq!(eixo.deslocar_visiveis(7, 4, 100), 11);
        assert_eq!(eixo.deslocar_visiveis(8, 4, 0), 9);
    }

    #[test]
    fn rolagem_sem_colunas_visiveis_nao_varre_toda_planilha() {
        let todas = (0..200u32).map(|i| (i, 0)).collect::<HashMap<_, _>>();
        let eixo = Eixo::novo(200, 122., &todas, None);
        assert_eq!(eixo.deslocar_visiveis(0, 0, 2), 0);
        assert_eq!(eixo.deslocar_visiveis(100, 4, -2), 4);
        let quase = (2..199u32).map(|i| (i, 0)).collect::<HashMap<_, _>>();
        let eixo = Eixo::novo(200, 122., &quase, None);
        assert_eq!(eixo.deslocar_visiveis(1, 0, 1), 199);
        assert_eq!(eixo.deslocar_visiveis(199, 0, -1), 1);
    }

    #[test]
    fn roda_ignora_blocos_ocultos_em_ambas_as_direcoes() {
        let medidas = HashMap::from([
            (0, 0), (1, 0), (5, 0), (6, 0), (7, 0), (12, 0)
        ]);
        let e = Eixo::novo(15, 28., &medidas, None);
        assert_eq!(e.proximo_visivel(0), Some(2));
        assert_eq!(e.anterior_visivel(1), None);
        assert_eq!(e.anterior_visivel(2), Some(2));
        assert_eq!(e.anterior_visivel(7), Some(4));
        assert_eq!(e.proximo_visivel(5), Some(8));
        assert_eq!(e.anterior_visivel(14), Some(14));
        assert_eq!(e.anterior_visivel(12), Some(11));
    }

    #[test]
    fn limite_inferior_equivale_a_varredura_reversa_com_ocultas() {
        let configuracoes = [
            HashMap::new(),
            HashMap::from([(9, 0)]),
            HashMap::from([(0, 0)]),
            HashMap::from([(7, 0), (8, 0), (9, 0)]),
            HashMap::from([(0, 0), (1, 0), (2, 0), (3, 0)]),
            HashMap::from([(0, 0), (1, 14), (8, 75), (9, 0)]),
            HashMap::from([(0, 0), (1, 0), (2, 0), (3, 0), (4, 0),
                (5, 0), (6, 0), (7, 0), (8, 0), (9, 0)]),
        ];
        for alturas in configuracoes {
            let eixo = Eixo::novo(10, 28.0, &alturas, None);
            for disponivel in [1., 8., 27., 28., 29., 56., 70., 90., 112., 280., 999.] {
                assert_eq!(eixo.inicio_da_cauda(f64::from(disponivel)),
                    inicio_reverso_anterior(&eixo, disponivel),
                    "Alturas {alturas:?}, canvas={disponivel}");
            }
        }
    }

    #[test]
    fn cauda_em_igualdade_exata_nao_inclui_linha_oculta_anterior() {
        let eixo = Eixo::novo(3, 12., &HashMap::from([(1, 0)]), None);
        // A última linha ocupa sozinha o viewport; a linha 1 está
        // oculta e não deve antecipar a origem ao índice 1.
        assert_eq!(eixo.inicio_da_cauda(12.), 2);
        assert_eq!(eixo.inicio_da_cauda(13.), 1);
        let com_zero_no_inicio = Eixo::novo(2, 12., &HashMap::from([(0, 0)]), None);
        assert_eq!(com_zero_no_inicio.inicio_da_cauda(12.), 1);
        assert_eq!(com_zero_no_inicio.inicio_da_cauda(13.), 0);
    }

    #[test]
    fn todas_as_combinacoes_pequenas_preservam_limite_antigo() {
        // 3^5 cenários determinísticos: linha oculta, altura padrão ou alta.
        for mascara in 0..243u32 {
            let mut codigo = mascara;
            let mut alturas = HashMap::new();
            for indice in 0..5 {
                let altura = match codigo % 3 { 0 => 0, 1 => 12, _ => 75 };
                codigo /= 3;
                if altura != 28 { alturas.insert(indice, altura); }
            }
            let eixo = Eixo::novo(5, 28., &alturas, None);
            for disp in [0., 1., 12., 13., 28., 29., 56., 75., 90., 150., 375.] {
                assert_eq!(eixo.inicio_da_cauda(f64::from(disp)),
                    inicio_reverso_anterior(&eixo, disp),
                    "Configuração {mascara}, altura disponível {disp}");
            }
        }
    }

    #[test]
    fn limite_de_dez_milhoes_com_muitas_linhas_ocultas() {
        let mut alturas = HashMap::new();
        for indice in 9_800_000..10_000_000 {
            alturas.insert(indice, 0);
        }
        let eixo = Eixo::novo(10_000_000, 28.0, &alturas, None);
        // Inclui as linhas ocultas da cauda sem fazer 200 mil passos.
        assert_eq!(eixo.inicio_da_cauda(100.0), 9_799_997);
        assert_eq!(eixo.inicio_da_cauda(28.0), 9_799_999);
        assert_eq!(eixo.inicio_da_cauda(1.0), 9_800_000);
        assert_eq!(eixo.inicio_da_cauda(eixo.total()), 0);
        assert_eq!(eixo.inicio_da_cauda(0.0), 9_999_999);
    }

    #[test]
    fn busca_proxima_dimensao_sem_varrer_dez_milhoes_de_linhas() {
        let mut wb = Workbook::new();
        wb.editar(0, |sheet| {
            for i in 0..100_000 { sheet.row_heights.insert(i, 0); }
        });
        let mut cache = EstadoEixos::default();
        assert_eq!(cache.visivel_apos_ocultar(&wb, 0, false, 0, 20), Some(100_000));
        assert_eq!(cache.visivel_apos_ocultar(&wb, 0, false, 50, 99_999), Some(100_000));
        wb.editar(0, |sheet| {
            sheet.row_heights.insert(100_000, 0);
        });
        assert_eq!(cache.visivel_apos_ocultar(&wb, 0, false, 0, 100_000), Some(100_001));
    }

    #[test]
    fn eixo_oculto_ate_o_fim_retrocede_para_anterior_visivel() {
        use feathersheet_core::celulas::refs::MAX_COLS;
        let mut wb = Workbook::new();
        wb.editar(0, |sheet| {
            for i in 8..MAX_COLS { sheet.col_widths.insert(i, 0); }
        });
        let mut cache = EstadoEixos::default();
        assert_eq!(cache.visivel_apos_ocultar(
            &wb, 0, true, 8, MAX_COLS - 1,
        ), Some(7));
        assert_eq!(cache.visivel_apos_ocultar(
            &wb, 0, true, 0, MAX_COLS - 1,
        ), None);
    }

    #[test]
    fn cache_de_rolagem_horizontal_reflete_ocultacoes_e_desfazer() {
        let mut wb = Workbook::new();
        let mut cache = EstadoEixos::default();
        assert_eq!(cache.coluna_apos_rolar(&wb, 0, 1, 0, 1), 2);
        wb.editar(0, |aba| {
            aba.col_widths.insert(2, 0);
            aba.col_widths.insert(3, 0);
        });
        assert_eq!(cache.coluna_apos_rolar(&wb, 0, 1, 0, 1), 4);
        assert_eq!(cache.coluna_apos_rolar(&wb, 0, 4, 0, -1), 1);
        wb.editar(0, |aba| {
            aba.definir_dimensao(true, 2, 122);
            aba.definir_dimensao(true, 3, 122);
        });
        assert_eq!(cache.coluna_apos_rolar(&wb, 0, 1, 0, 1), 2);
    }

    #[test]
    fn cache_invalida_quando_a_versao_ou_a_aba_muda() {
        let mut wb = Workbook::new();
        let mut cache = EstadoEixos::default();
        let (linhas, _) = cache.para(&wb, 0, 10_000_000, 16_384, 28., 122.).unwrap();
        assert_eq!(linhas.tamanho(0), 28.);
        wb.editar(0, |s| { s.row_heights.insert(0, 0); });
        let (linhas, _) = cache.para(&wb, 0, 10_000_000, 16_384, 28., 122.).unwrap();
        assert_eq!(linhas.proximo_visivel(0), Some(1));
        cache.limpar();
        assert!(cache.cache.is_none());
    }

    #[test]
    fn ocultas_ate_o_fim_param_a_posicao_e_saltam_na_rolagem() {
        let e = Eixo::novo(100_000_000, 28., &HashMap::new(), Some(29));
        assert_eq!(e.tamanho(28), 28.0);
        assert_eq!(e.tamanho(29), 0.0);
        assert_eq!(e.tamanho(99_999_999), 0.0);
        assert_eq!(e.total(), 29.0 * 28.0);
        assert_eq!(e.proximo_visivel(29), None);
        assert_eq!(e.anterior_visivel(50_000_000), Some(28));
        assert_eq!(e.deslocar_visiveis(20, 0, 50), 28);
    }
}

