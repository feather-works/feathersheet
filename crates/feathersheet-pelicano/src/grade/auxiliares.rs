//! Cálculo de viewport, posição, seleção e validação da grade.
use super::*;

impl Grade<'_> {

    /// Opções da lista válida na célula ativa. Faz leitura sob demanda para
    /// refletir mudanças nas células de origem sem manter cache obsoleto.
    /// Itens da lista suspensa da célula ativa: a validação manda; sem ela,
    /// os valores da coluna pedidos com Alt+↓.
    pub(super) fn opcoes_lista(&self) -> Option<Vec<String>> {
        self.opcoes_validacao().or_else(|| self.opcoes_coluna.map(<[String]>::to_vec))
    }

    /// As opções da lista de validação da célula ativa: fixas, de um
    /// intervalo (desta ou de outra aba), de um nome ou de `INDIRETO`. A
    /// regra mora no motor (`planilha::lista_validacao`).
    pub(super) fn opcoes_validacao(&self) -> Option<Vec<String>> {
        lista_suspensa::opcoes_validacao(self.workbook, self.aba, self.selecionada.0, self.selecionada.1)
    }

    /// Valores do checkbox, se a regra ativa da seleção for Caixa.
    /// A célula cuja caixa de seleção está sob `p` (unidades da grade, sem
    /// zoom), com uma folga de 3 em volta da caixa; fora dela o clique
    /// seleciona a célula normalmente.
    pub(super) fn caixa_no_ponto(&self, p: Point, viewport: Size) -> Option<(u32, u32)> {
        if self.mostrar_formulas || self.rascunho.is_some() { return None; }
        let aba = self.workbook.aba(self.aba)?;
        let regras = crate::caixas::Regras::da(&aba.sheet);
        if !regras.tem_caixa() { return None; }
        let (colunas, _) = self.colunas_visiveis(viewport.width);
        let (linhas, _) = self.linhas_visiveis(viewport.height);
        let &(c, x, w) = colunas.iter().find(|(_, x, w)| p.x >= *x && p.x < *x + *w)?;
        let &(r, y, h) = linhas.iter().find(|(_, y, h)| p.y >= *y && p.y < *y + *h)?;
        let (m, d) = regras.caixa_em(r, c)?;
        crate::caixas::estado(&aba.sheet, r, c, m, d)?;
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        let meio = super::LADO_CAIXA / 2.0 + 3.0;
        ((p.x - cx).abs() <= meio && (p.y - cy).abs() <= meio).then_some((r, c))
    }

    /// Retorna a posição lógica da célula selecionada, limitada ao viewport
    /// visível. Usa larguras/alturas reais, inclusive linhas e colunas ocultas.
    pub(super) fn retangulo_celula_ativa(&self, viewport: Size) -> Option<(f32, f32, f32, f32)> {
        let (r,c) = self.selecionada;
        let (x,y,w,h) = self.posicao_da_celula(r,c,viewport)?;
        // A linha padrão do Excel (15 pt) tem ~18 px: a seta precisa caber nela.
        if w < 24.0 || h < 10.0 { return None; }
        Some((x,y,w,h))
    }

    /// Maior primeira linha que ainda mantém as linhas do final da planilha
    /// na área visível. A altura do Canvas vem em pixels físicos e considera
    /// zoom, cabeçalho e altura de linhas congeladas.
    ///
    /// Antes: varredura reversa (até 10 milhões de leituras se o fim da
    /// planilha estivesse oculto). Agora: busca por prefixos de alturas no
    /// mesmo índice esparso usado pela renderização/hit-test da grade.
    pub(super) fn limite_inferior_rolagem(&self, altura_canvas: f32) -> u32 {
        let zoom = self.zoom.max(0.01);
        // A barra de rolagem horizontal tem faixa própria: a última linha
        // encosta nela, sem ficar por baixo (como a XFD na vertical).
        let mut disponivel = (altura_canvas / zoom - CABECALHO_COLUNA
            - super::barras::ESPESSURA / zoom).max(0.0);
        let altura_fixa = (0..self.congelar_linhas.min(30))
            .map(|r| altura(self.workbook, self.aba, r).max(0.0))
            .sum::<f32>().min(disponivel * 0.70);
        disponivel = (disponivel - altura_fixa).max(0.0);
        if disponivel <= 0.0 {
            return MAX_ROWS - 1;
        }
        let mut cache = self.eixos.borrow_mut();
        cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ).map(|(linhas, _)| linhas.inicio_da_cauda(f64::from(disponivel)))
            .unwrap_or(MAX_ROWS - 1)
    }

    /// Largura lógica onde as colunas roláveis cabem: tira o cabeçalho das
    /// linhas, as colunas congeladas e a barra de rolagem vertical.
    fn largura_rolavel(&self, largura_canvas: f32) -> f32 {
        let zoom = self.zoom.max(0.01);
        let disponivel = (largura_canvas / zoom - CABECALHO_LINHA).max(0.0);
        let largura_fixa = (0..self.congelar_colunas.min(30))
            .map(|c| largura(self.workbook, self.aba, c).max(0.0))
            .sum::<f32>().min(disponivel * 0.70);
        (disponivel - largura_fixa - super::barras::ESPESSURA / zoom).max(0.0)
    }

    /// A coluna mais à direita que pode ser a origem: com ela, a ÚLTIMA (XFD)
    /// encosta na barra de rolagem. Se a cauda não cabe certinha, a origem é a
    /// coluna que vai aparecer cortada (ver [`Self::corte_da_origem`]), como no Excel.
    pub(super) fn limite_direito_rolagem(&self, largura_canvas: f32) -> u32 {
        let disponivel = self.largura_rolavel(largura_canvas);
        if disponivel <= 0.0 {
            return MAX_COLS - 1;
        }
        let mut cache = self.eixos.borrow_mut();
        cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ).map(|(_, colunas)| {
            let inicio = colunas.inicio_da_cauda(f64::from(disponivel));
            let cauda = colunas.total() - colunas.inicio(inicio);
            if inicio > self.congelar_colunas && cauda + 0.5 < f64::from(disponivel) {
                colunas.anterior_visivel(inicio - 1).filter(|c| *c >= self.congelar_colunas).unwrap_or(inicio)
            } else {
                inicio
            }
        }).unwrap_or(MAX_COLS - 1)
    }

    /// Quanto da coluna de origem fica escondido à esquerda (px lógicos): só no
    /// fim da planilha, para a XFD encostar na barra sem sobrar área vazia.
    pub(super) fn corte_da_origem(&self, largura_canvas: f32, origem: u32) -> f32 {
        if origem != self.limite_direito_rolagem(largura_canvas) {
            return 0.0;
        }
        let disponivel = f64::from(self.largura_rolavel(largura_canvas));
        let mut cache = self.eixos.borrow_mut();
        cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ).map(|(_, colunas)| {
            let cauda = colunas.total() - colunas.inicio(origem);
            (cauda - disponivel).clamp(0.0, f64::from(colunas.tamanho(origem)) - 1.0) as f32
        }).unwrap_or(0.0).max(0.0)
    }

    /// A coluna de origem de fato desenhada (ver [`Self::limite_direito_rolagem`]).
    pub(super) fn primeira_coluna_visivel(&self, largura_canvas: f32) -> u32 {
        self.coluna_inicio.max(self.congelar_colunas)
            .min(self.limite_direito_rolagem(largura_canvas).max(self.congelar_colunas))
    }

    /// Posição normalizada em qualquer rota: desenho, hit-test ou scroll.
    pub(super) fn primeira_linha_visivel(&self, altura_canvas: f32) -> u32 {
        self.linha_inicio.max(self.congelar_linhas)
            .min(self.limite_inferior_rolagem(altura_canvas).max(self.congelar_linhas))
    }

    /// A roda do mouse usa a origem real e salta as faixas ocultas.
    /// Sem isso, cada giro avançava três índices lógicos, mesmo que um
    /// milhão de linhas seguidas estivesse invisível (altura zero).
    pub(super) fn origem_apos_rolar(&self, altura_canvas: f32, delta: i32) -> u32 {
        let inferior = self.limite_inferior_rolagem(altura_canvas)
            .max(self.congelar_linhas);
        let partida = self.linha_inicio.max(self.congelar_linhas).min(inferior);
        let destino = (i64::from(partida) + i64::from(delta))
            .clamp(i64::from(self.congelar_linhas), i64::from(inferior)) as u32;
        if delta == 0 { return destino; }
        let mut cache = self.eixos.borrow_mut();
        let Some((linhas, _)) = cache.para(
            self.workbook, self.aba, MAX_ROWS, MAX_COLS,
            ALTURA_PADRAO, LARGURA_PADRAO,
        ) else { return destino; };
        if delta > 0 {
            // Não caminhar por um bloco oculto até a próxima linha visível.
            linhas.proximo_visivel(destino).unwrap_or(inferior)
                .min(inferior)
        } else {
            // Respeitar o começo da região rolável e não voltar à faixa fixa.
            let primeiro = linhas.proximo_visivel(self.congelar_linhas)
                .unwrap_or(self.congelar_linhas).min(inferior);
            linhas.anterior_visivel(destino).unwrap_or(primeiro)
                .clamp(primeiro, inferior)
        }
    }

    /// Mapeia ponto do viewport para célula absoluta, respeitando alturas
    /// e larguras configuradas em Rust, inclusive rolagem horizontal.
    pub fn celula_no_ponto(&self, p: Point, tamanho: Size) -> Option<(u32, u32)> {
        // O Sheet usa pixels LÓGICOS; índices de linha/coluna continuam
        // pertencendo exclusivamente ao motor de planilhas.
        let viewport = feather_draw::Viewport::de_escala_logica(
            feather_draw::Unidade::PixelLogico,
            self.zoom,
            feather_draw::Ponto::novo(0.0, 0.0),
            feather_draw::Ponto::novo(0.0, 0.0),
            1.0,
        ).ok()?;
        let local = viewport.documento(feather_draw::Ponto::novo(p.x, p.y));
        let p = Point::new(local.x, local.y);
        let tamanho = Size::new(tamanho.width / viewport.escala(),
                                tamanho.height / viewport.escala());
        if p.x < CABECALHO_LINHA || p.y < CABECALHO_COLUNA ||
            p.x >= tamanho.width || p.y >= tamanho.height { return None; }
        self.ponto_para_celula(p,tamanho)
    }

    pub(super) fn texto(&self, r: u32, c: u32) -> String {
        if (r,c)==self.selecionada {
            if let Some(t)=self.rascunho {return t.to_owned();}
        }
        // Mesmo contrato do _showFormulas do TypeScript: texto bruto para
        // cada célula, sem avaliar fórmulas, converter números ou aplicar formato.
        if self.mostrar_formulas {
            return self.workbook.aba(self.aba)
                .map(|aba| feathersheet_core::celulas::locale::apresentar_formula(
                    &crate::idioma::mostrar_formula(&aba.sheet.get_raw(r, c)), &feathersheet_core::get_locale_config()))
                .unwrap_or_default();
        }
        // O desenho nunca calcula: conta pendente aparece como "…" e o app
        // calcula fora daqui (`App::calcular_visiveis`).
        if self.workbook.calculo_pendente(self.aba, r, c) {
            return "…".into();
        }
        let estilo = self.workbook.aba(self.aba).and_then(|a|a.sheet.estilo_resolvido(r,c));
        let valor = self.workbook.valor(self.aba, r, c);
        format_value(&valor, estilo.as_ref())
    }

    /// Cor da seção e preenchimento do `*` do formato personalizado da
    /// célula (`[Vermelho]`, `* #,##0`). O texto continua vindo de `texto`.
    pub(super) fn formato_extra(&self, r: u32, c: u32,
        estilo: Option<&feathersheet_core::celulas::style::Style>)
        -> Option<feathersheet_core::celulas::formato_personalizado::Formatado>
    {
        let codigo = estilo?.code.as_deref()?;
        if self.mostrar_formulas || ((r, c) == self.selecionada && self.rascunho.is_some())
            || self.workbook.calculo_pendente(self.aba, r, c) { return None; }
        // `-0` já cai na seção do zero no motor (-0.0 < 0.0 é falso).
        let valor = self.workbook.valor(self.aba, r, c);
        feathersheet_core::celulas::formato_personalizado::formatar_completo(&valor, codigo)
            .filter(|f| f.cor.is_some() || f.preencher.is_some())
    }

    pub(super) fn alinhar(&self, r:u32,c:u32) -> alignment::Horizontal {
        // O modo de inspeção sempre alinha a fórmula à esquerda, tal qual TS.
        if self.mostrar_formulas { return pelicano::Align::Start; }
        if self.workbook.calculo_pendente(self.aba, r, c) { return pelicano::Align::Start; }
        let valor = self.workbook.valor(self.aba,r,c);
        let estilo = self.workbook.aba(self.aba).and_then(|a|a.sheet.estilo_resolvido(r,c));
        match effective_align(&valor,estilo.as_ref()) {
            Align::Right => pelicano::Align::End,
            Align::Center => pelicano::Align::Center,
            _ => pelicano::Align::Start,
        }
    }
}
