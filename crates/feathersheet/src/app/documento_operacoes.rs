//! Seleção, edição, transferência e abertura/salvamento de documentos.
use super::*;

impl App {
    /// A fórmula como a barra mostra: separadores e nomes das funções no
    /// idioma da interface (de-para de `funcoes.<idioma>.json`).
    pub(super) fn formula_para_mostrar(&self, raw: &str) -> String {
        // Imagem na célula: a barra fica vazia, como no Excel (não o base64).
        if feathersheet_core::funcoes::imagem_base64::eh_imagem_na_celula(raw) { return String::new(); }
        let nomes = crate::idioma::mostrar_formula(raw);
        feathersheet_core::celulas::locale::apresentar_formula(&nomes, &self.regionalizacao())
    }

    /// O texto editado para gravar. Sem mudança em relação ao que a barra
    /// mostrou, fica o original (trocar SUM por SOMA na tela não é edição).
    pub(super) fn formula_para_gravar(&self, editado: &str, antes: &str) -> String {
        if !antes.is_empty() && self.formula_para_mostrar(antes) == editado { return antes.to_owned(); }
        let motor = crate::idioma::formula_para_motor(editado);
        feathersheet_core::celulas::locale::normalizar_formula(&motor, &self.regionalizacao())
    }

    pub(super) fn regionalizacao(&self) -> feathersheet_core::LocaleConfig {
        feathersheet_core::LocaleConfig::embutida(&self.config.idioma)
            .unwrap_or_else(feathersheet_core::LocaleConfig::pt_br)
    }

    /// Seleciona `(r, c)` e rola o mínimo para ela caber inteira na tela
    /// (teclado, Ir para, Localizar, comandos).
    pub(super) fn trocar_selecao(&mut self, r: u32, c: u32) {
        self.trocar_selecao_com(r, c, None);
    }

    /// Clique do mouse: a célula clicada já está desenhada, nem que em
    /// parte, e a tela nunca rola (antes rolava com zoom abaixo de 100%).
    pub(super) fn trocar_selecao_por_clique(&mut self, r: u32, c: u32) {
        self.trocar_selecao_com(r, c, Some((r, c)));
    }

    /// A barra fx com o conteúdo da célula ativa (quem reescreve a célula
    /// sem trocar a seleção, como a dinâmica, chama isto).
    pub(super) fn recarregar_barra_fx(&mut self) {
        // Fórmula oculta (Formatar células → Proteção) com a aba protegida: a barra fica vazia.
        self.edicao = self.workbook.aba(self.folha)
            .filter(|a| !a.sheet.formula_oculta(self.linha, self.coluna))
            .map(|a| self.formula_para_mostrar(&a.sheet.get_raw(self.linha, self.coluna))).unwrap_or_default();
    }

    fn trocar_selecao_com(&mut self, r: u32, c: u32, clique: Option<(u32, u32)>) {
        self.selecoes_extras.clear();
        // Célula escolhida: o gráfico deixa de estar selecionado (Delete limpa a célula).
        self.desselecionar_grafico();
        self.edicao_inline = None;
        self.multilinha = None;
        self.editor_formula.encerrar();
        self.lista_aberta = false;
        self.lista_coluna = None;
        self.lista_pagina = 0;
        let r = r.min(MAX_ROWS - 1);
        let c = c.min(MAX_COLS - 1);
        let mescla = self.workbook.aba(self.folha)
            .and_then(|aba| mesclas::ler(&aba.sheet).into_iter().find(|a| a.contains(r, c)));
        let (r, c, fim) = match mescla {
            Some(a) => (a.r0, a.c0, (a.r1, a.c1)),
            None => (r, c, (r, c)),
        };
        self.linha = r;
        self.coluna = c;
        self.fim_selecao = fim;
        self.tabela_popup_filtro = false;
        self.tabela_popup_totais = false;
        // A contextual deve seguir a tabela da célula ativa, mesmo se o
        // usuário alternar entre duas tabelas sem clicar de novo na aba.
        let tabela_atual=tabelas::sob_celula(
            &self.workbook,self.folha,self.linha,self.coluna);
        let novo_id=tabela_atual.as_ref()
            .and_then(|t| t["id"].as_str().map(str::to_owned));
        if self.tabela_editando != novo_id {
            self.tabela_renomeando = false;
            self.tabela_nome_novo=tabela_atual.as_ref()
                .and_then(|t|t["nome"].as_str()).unwrap_or("").to_owned();
            self.tabela_editando=novo_id;
        }
        if self.faixa_ativa == AbaFaixa::Dinamica && !self.dinamica_na_faixa() {
            self.faixa_ativa = AbaFaixa::Inicio;
        }
        if self.faixa_ativa == AbaFaixa::Tabela && self.tabela_editando.is_none() {
            self.faixa_ativa = AbaFaixa::Inicio;
        }
        self.recarregar_barra_fx();
        self.endereco = cell_name(self.linha, self.coluna);
        self.endereco_digitado = false;
        self.manter_selecao_a_vista(clique);
    }

    /// Rola a grade para a célula ativa aparecer. Com o tamanho do Canvas
    /// conhecido (já desenhado), a conta é a da própria grade: zoom,
    /// alturas/larguras reais, ocultas e painéis congelados. Sem ele (antes do
    /// primeiro desenho), cai na estimativa da configuração.
    fn manter_selecao_a_vista(&mut self, clique: Option<(u32, u32)>) {
        use crate::grade::Exigencia;
        let ((r, c), exigencia) = match clique {
            Some(p) => (p, Exigencia::Parcial),
            None => ((self.linha, self.coluna), Exigencia::Inteira),
        };
        if let Some(rolou) = self.mostrar_celula(r, c, exigencia) {
            if rolou {
                self.ancorar_rodape = self.linha == MAX_ROWS - 1;
            } else if self.linha == MAX_ROWS - 1 {
                self.ancorar_rodape = true;
            }
            return;
        }
        let (linhas_fixas, colunas_fixas) = self.congelamento_atual();
        if self.linha >= linhas_fixas &&
            (self.linha < self.linha_inicio ||
             self.linha >= self.linha_inicio.saturating_add(self.config.linhas_visiveis))
        {
            self.linha_inicio = self.linha.saturating_sub(2).max(linhas_fixas);
            self.ancorar_rodape = self.linha == MAX_ROWS - 1;
        } else if self.linha == MAX_ROWS - 1 {
            self.ancorar_rodape = true;
        }
        if self.coluna >= colunas_fixas &&
            (self.coluna < self.coluna_inicio ||
             self.coluna >= self.coluna_inicio.saturating_add(self.config.colunas_visiveis))
        {
            self.coluna_inicio = self.coluna.saturating_sub(2).max(colunas_fixas);
        }
    }
    /// Rola o mínimo para `(r, c)` aparecer, com a geometria real da grade.
    /// `None` antes do primeiro desenho (sem o tamanho do Canvas); senão diz
    /// se a origem vertical mudou.
    pub(super) fn mostrar_celula(&mut self, r: u32, c: u32, exigencia: crate::grade::Exigencia) -> Option<bool> {
        let tela = self.eixos_grade.borrow().tamanho_canvas
            .filter(|t| t.width > 0.0 && t.height > 0.0)?;
        let (linha, coluna) = self.grade().origem_para_mostrar(r, c, tela, exigencia);
        if let Some(l) = linha { self.linha_inicio = l; }
        if let Some(c) = coluna { self.coluna_inicio = c; }
        Some(linha.is_some())
    }

    /// Linhas e colunas roláveis que cabem inteiras na tela, com o zoom e as
    /// medidas reais (passo do Page Down/Up). Antes do primeiro desenho, a
    /// estimativa da configuração.
    pub(super) fn pagina_da_grade(&self) -> (u32, u32) {
        let tela = self.eixos_grade.borrow().tamanho_canvas;
        match tela.filter(|t| t.width > 0.0 && t.height > 0.0) {
            Some(tela) => self.grade().pagina(tela),
            None => (self.config.linhas_visiveis, self.config.colunas_visiveis),
        }
    }

    /// Com fórmula aberta, escreve a faixa apontada (colunas ou linhas
    /// inteiras pelos cabeçalhos), com o nome da aba quando é outra.
    /// `None` quando não há fórmula para apontar: o clique seleciona.
    pub(super) fn apontar_faixa(&mut self, ancora: (u32, u32), ponto: (u32, u32)) -> Option<Task<Msg>> {
        if !self.editor_formula.ativo || !self.edicao.starts_with('=') { return None; }
        let outra = self.editor_formula.origem.and_then(|(origem, _, _)|
            (origem != self.folha).then(|| self.workbook.nomes().get(self.folha).cloned()).flatten());
        if !self.editor_formula.referencia_faixa(&mut self.edicao, ancora, ponto, outra.as_deref()) {
            return None;
        }
        // shortcut: o text_input do pelicano não recebe a posição do cursor por Task: ele fica onde
        // estava. Melhorar quando o pelicano tiver mover cursor por id.
        Some(Task::focus("feathersheet.fx"))
    }

    pub(super) fn area_selecionada(&self) -> Rect {
        let selecionada = Rect::new(self.linha, self.coluna,
            self.fim_selecao.0, self.fim_selecao.1).normalized();
        // Coluna ou linha inteira não estica pelas mescladas, como no Excel:
        // senão escolher B com A1:B1 mesclada levava A junto (largura, ocultar…).
        if self.selecao_e_colunas_inteiras() || self.selecao_e_linhas_inteiras() {
            return selecionada;
        }
        self.workbook.aba(self.folha)
            .map(|aba| mesclas::expandir(&mesclas::ler(&aba.sheet), selecionada.clone()))
            .unwrap_or(selecionada)
    }

    /// Posição alcançada por Ctrl+seta, respeitando o bloco contínuo de dados.
    /// Em uma célula vazia, primeiro procura o próximo dado; em um bloco,
    /// avança até a primeira célula vazia ou até o limite usado da aba.
    /// Ctrl+seta sai da célula ativa; com Shift, da PONTA da seleção (a que
    /// se move), como no Excel: Ctrl+Shift+→ e depois Ctrl+Shift+↓ pega o
    /// bloco inteiro, em vez de encolher para a coluna da célula ativa.
    pub(super) fn destino_ctrl_seta(&mut self, dr: i32, dc: i32, shift: bool) -> (u32, u32) {
        let origem = if shift { self.fim_selecao } else { (self.linha, self.coluna) };
        self.destino_ctrl_seta_de(origem, dr, dc)
    }
    /// O salto do motor (`borda_dos_dados`, o `edgeJump` do Excel): no meio
    /// de um bloco vai até a borda; numa borda ou no vazio, até o próximo
    /// bloco — sem montar o texto de cada célula no caminho.
    pub(super) fn destino_ctrl_seta_de(&mut self, origem:(u32,u32), dr:i32, dc:i32)->(u32,u32) {
        let Some(aba)=self.workbook.aba_mut(self.folha) else {return origem};
        // Ocultas até o fim: o salto para na última visível (Ctrl+↓ não
        // some na faixa oculta).
        let ultima_l=aba.sheet.linhas_ocultas_desde.map_or(MAX_ROWS-1, |n| n.saturating_sub(1)).max(origem.0);
        let ultima_c=aba.sheet.colunas_ocultas_desde.map_or(MAX_COLS-1, |n| n.saturating_sub(1)).max(origem.1);
        aba.sheet.borda_dos_dados(origem.0, origem.1, dr, dc, ultima_l, ultima_c)
    }

    /// Uma seta: a próxima linha/coluna visível na direção, pulando as
    /// ocultas como o Excel. Sem visível adiante, fica onde está.
    pub(super) fn passo_visivel(&self, origem:(u32,u32), dr:i64, dc:i64)->(u32,u32) {
        let andar=|v:u32, d:i64, limite:u32, oculta:&dyn Fn(u32)->bool| -> u32 {
            if d==0 { return v; }
            let mut alvo=(v as i64+d).clamp(0, limite as i64-1) as u32;
            while alvo!=v && oculta(alvo) {
                let proximo=(alvo as i64+d.signum()).clamp(0, limite as i64-1) as u32;
                if proximo==alvo { return v; }
                alvo=proximo;
            }
            if oculta(alvo) && alvo!=v { v } else { alvo }
        };
        let nada=|_:u32| false;
        let Some(aba)=self.workbook.aba(self.folha) else {
            return (andar(origem.0, dr, MAX_ROWS, &nada), andar(origem.1, dc, MAX_COLS, &nada));
        };
        let s=&aba.sheet;
        // A faixa oculta do fim encerra a caminhada de uma vez (não percorre
        // 100 milhões de linhas uma a uma). Já dentro dela (Ir para), anda
        // sem pular.
        let (limite_l, pula_l)=match s.linhas_ocultas_desde {
            Some(n) if origem.0<n => (n, true),
            Some(_) => (MAX_ROWS, false),
            None => (MAX_ROWS, true),
        };
        let (limite_c, pula_c)=match s.colunas_ocultas_desde {
            Some(n) if origem.1<n => (n, true),
            Some(_) => (MAX_COLS, false),
            None => (MAX_COLS, true),
        };
        let linha_oculta=|r:u32| pula_l && s.linha_oculta(r);
        let coluna_oculta=|c:u32| pula_c && s.coluna_oculta(c);
        (andar(origem.0, dr, limite_l, &linha_oculta),
         andar(origem.1, dc, limite_c, &coluna_oculta))
    }

    pub(super) fn copiar_selecao(&mut self) -> Option<String> {
        self.confirmar();
        if self.tem_varias_areas() {
            self.status = tr!("Copiar e recortar não funcionam com várias áreas selecionadas").into();
            return None;
        }
        let area = self.area_selecionada();
        let aba = self.workbook.aba(self.folha)?;
        // A renderização combina estilos da aba, coluna, linha e célula.
        // Guardar o estilo mesclado garante que o destino mantenha a aparência.
        let texto_do_valor = |r: u32, c: u32| match self.workbook.valor(self.folha, r, c) {
            feathersheet_core::Value::Empty => String::new(),
            feathersheet_core::Value::Number(n) =>
                feathersheet_core::celulas::number::to_js_string(n),
            feathersheet_core::Value::Text(t) => t,
            feathersheet_core::Value::Bool(true) => "VERDADEIRO".into(),
            feathersheet_core::Value::Bool(false) => "FALSO".into(),
            feathersheet_core::Value::Error(e) => e.code.as_str().into(),
        };
        // Vizinha de matriz derramada, sem a âncora na cópia: vai o valor,
        // como no Excel. Com a âncora junto, vai só a fórmula, que derrama
        // de novo no destino.
        let derramada_sem_ancora = |r: u32, c: u32| aba.sheet.area_derramada_em(r, c)
            .is_some_and(|a| (a.r0, a.c0) != (r, c) && !area.contains(a.r0, a.c0));
        let celulas: Vec<Vec<Snap>> = (area.r0..=area.r1).map(|r| {
            (area.c0..=area.c1).map(|c| Snap {
                raw: if derramada_sem_ancora(r, c) { texto_do_valor(r, c) } else { aba.sheet.get_raw(r, c) },
                style: aba.sheet.estilo_mesclado(r, c),
            }).collect()
        }).collect();
        // Colar só valores não pode transportar a fórmula original: calcula
        // os resultados no instante da cópia, inclusive referências entre abas.
        let valores: Vec<Vec<String>> = celulas.iter().enumerate().map(|(dr, linha)| {
            linha.iter().enumerate().map(|(dc, celula)| {
                let (r, c) = (area.r0 + dr as u32, area.c0 + dc as u32);
                if celula.raw.starts_with('=') || (celula.raw.is_empty() && aba.sheet.area_derramada_em(r, c).is_some()) {
                    texto_do_valor(r, c)
                } else {
                    celula.raw.clone()
                }
            }).collect()
        }).collect();
        let linhas: Vec<String> = celulas.iter().map(|linha| {
            linha.iter().map(|celula| {
                let raw = &celula.raw;
                if raw.contains(['\t', '\n', '\r', '"']) {
                    format!("\"{}\"", raw.replace('"', "\"\""))
                } else { raw.clone() }
            }).collect::<Vec<_>>().join("\t")
        }).collect();
        let texto = linhas.join("\n");
        let larguras = (area.c0..=area.c1)
            .map(|c| aba.sheet.col_widths.get(&c).copied()).collect();
        self.area_copiada = Some(AreaCopiada {
            texto: texto.clone(), celulas, valores, larguras,
            origem: (area.r0, area.c0), recortada: false,
        });
        Some(texto)
    }
    /// Captura somente estilos resolvidos. Pressionar o botão novamente desativa.
    pub(super) fn capturar_pincel(&mut self) {
        if self.pincel_formatacao.take().is_some() {
            self.status = tr!("Pincel de formatação cancelado").into();
            return;
        }
        self.confirmar();
        let area = self.area_selecionada();
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let estilos = (area.r0..=area.r1).map(|r| {
            (area.c0..=area.c1)
                .map(|c| aba.sheet.estilo_mesclado(r, c))
                .collect::<Vec<Option<EstiloParcial>>>()
        }).collect();
        self.pincel_formatacao = Some(PincelFormatacao { estilos });
        self.status = tr!("Pincel ativo: clique ou arraste nas células de destino. Esc cancela.").into();
    }

    /// Pinta um intervalo em uma única transação. A operação nunca transfere
    /// valores ou fórmulas; permite desfazer em um único Ctrl+Z.
    pub(super) fn aplicar_pincel(&mut self) {
        let Some(pincel) = self.pincel_formatacao.take() else { return };
        let altura = pincel.estilos.len();
        let largura = pincel.estilos.first().map_or(0, Vec::len);
        let plano = match PlanoColagem::calcular(&self.area_selecionada(), altura, largura) {
            Ok(p) => p,
            Err(erro) => {
                self.status = trf!("Pincel: {erro}. Esc cancela.");
                self.pincel_formatacao = Some(pincel);
                return;
            }
        };
        let mut mudou = false;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for dr in 0..plano.altura {
                for dc in 0..plano.largura {
                    let r = plano.r0 + dr as u32;
                    let c = plano.c0 + dc as u32;
                    let mut destino = sheet.snapshot(r, c)
                        .unwrap_or_else(|| Snap::texto(""));
                    destino.style = pincel.estilos[dr % altura][dc % largura].clone();
                    let next = if destino.raw.is_empty() && destino.style.is_none() {
                        None
                    } else {
                        Some(destino)
                    };
                    mudou |= sheet.apply(r, c, next, true);
                }
            }
            sheet.fechar_transacao();
        });
        self.alterado |= mudou;
        self.trocar_selecao(plano.r0, plano.c0);
        let area = plano.area();
        self.fim_selecao = (area.r1, area.c1);
        if plano.altura > 1 || plano.largura > 1 {
            self.endereco = format!(
                "{}:{}", cell_name(area.r0, area.c0), cell_name(area.r1, area.c1)
            );
        }
        self.status = trf!(
            "Pincel aplicado em {} células, sem modificar valores/fórmulas",
            plano.altura * plano.largura
        );
    }

    /// Retorna falso quando a regra Recusar bloqueia o valor. Nesse caso a
    /// edição e o conteúdo digitado permanecem ativos para correção.
    pub(super) fn confirmar(&mut self) -> bool {
        if !self.editor_formula.ativo {
            self.edicao_inline = None;
            self.multilinha = None;
            return true;
        }
        // Alerta de validação aberto: espera a resposta (Repetir, Sim, OK…).
        if self.validacao_alerta.is_some() { return false; }
        let (folha, linha, coluna) = self.editor_formula.origem
            .unwrap_or((self.folha, self.linha, self.coluna));
        let antes = self.workbook.aba(folha)
            .map(|a| a.sheet.get_raw(linha, coluna)).unwrap_or_default();
        let canonica = self.formula_para_gravar(&self.edicao, &antes);
        // Valor fora da validação abre o alerta de erro (Parar, Aviso ou
        // Informações); a edição continua até a resposta. `validacao_aceita`
        // é o Sim/OK do alerta: grava sem perguntar de novo.
        if antes != canonica && !self.validacao_aceita {
            match self.alerta_validacao(folha, linha, coluna, &canonica) {
                Ok(None) => {}
                Ok(Some(alerta)) => {
                    self.validacao_alerta = Some(validacao_dialogo::AlertaPendente { alerta });
                    return false;
                }
                Err(erro) => {
                    self.status = format!("{}: {}", cell_name(linha, coluna), erro);
                    return false;
                }
            }
        }
        self.editor_formula.encerrar();
        self.edicao_inline = None;
        self.multilinha = None;
        if antes != canonica {
            // Digitar numa célula tira a marca do Ctrl+C, como no Excel.
            self.marca_copia = None;
            // Dimensionamento explícito: textos Alt+Enter não podem ocupar a
            // linha seguinte. Respeita altura personalizada se já for maior.
            let linhas = canonica.split('\n').count();
            self.workbook.escrever_celula(folha, linha, coluna, canonica);
            if linhas > 1 {
                let altura = ((linhas as f32 * 18.0 + 10.0).ceil() as u32).clamp(28, 400);
                let atual = self.workbook.aba(folha)
                    .and_then(|a| a.sheet.row_heights.get(&linha).copied())
                    .unwrap_or(crate::grade::ALTURA_PADRAO as u32);
                if altura > atual {
                    self.workbook.editar(folha, |sheet| {
                        sheet.definir_dimensao(false, linha, altura);
                    });
                }
            }
            if !self.edicao.is_empty() {
                if let Err(e) = tabelas::crescer(&mut self.workbook, folha, linha, coluna) {
                    self.status = e;
                }
            }
            self.alterado = true;
            self.status = trf!("{} atualizado", cell_name(linha, coluna));
            self.atualizar_circulos();
        }
        if folha != self.folha {
            self.folha = folha;
            self.nome_aba = self.workbook.nomes().get(folha).cloned().unwrap_or_default();
            self.trocar_selecao(linha, coluna);
        }
        true
    }
    // Para XLSX, valida os bytes compactados antes de alocar o buffer.
    // CSV/FSHZ preservam os codecs existentes e os testes por bytes.
    /// Lê o arquivo e já calcula as fórmulas em paralelo — roda na thread de
    /// abertura, então a janela não espera: a planilha chega calculada.
    pub(super) fn decodificar_caminho(path: &Path, separador: Option<char>) -> Result<(Workbook, usize), String> {
        let (mut workbook, folha) = Self::decodificar_sem_calcular(path, separador)?;
        feathersheet_io::progresso::fase(feathersheet_io::progresso::Fase::Calculando);
        workbook.calcular_em_paralelo();
        // Ainda na thread da abertura: os blocos de números que cabem em 1, 2 ou 4 bytes
        // sem perda encolhem (o 2026_03, 13,7 M linhas: 4,45 → 2,05 GB, 1,3 s).
        let t = Instant::now();
        let (antes, depois) = workbook.compactar_colunas();
        if antes > 0 {
            feathersheet_core::diario::anotar(trf!("[abrir] colunas compactadas: {} → {} MB · {} ms",
                antes >> 20, depois >> 20, t.elapsed().as_millis()));
        }
        // O que a leitura usou de passagem (XML, textos, lotes) volta ao sistema: sem isso o
        // RSS ficava no pico da abertura.
        feathersheet_core::paralelismo::devolver_memoria_ao_sistema();
        Ok((workbook, folha))
    }

    fn decodificar_sem_calcular(path: &Path, separador: Option<char>) -> Result<(Workbook, usize), String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("");
        if ext.eq_ignore_ascii_case("csv") || ext.eq_ignore_ascii_case("tsv") {
            return Ok((feathersheet_io::csv_workbook::importar_csv_tsv_arquivo(
                path, ext.eq_ignore_ascii_case("tsv"), separador,
            )?, 0));
        }
        if ext.eq_ignore_ascii_case("parquet") {
            feathersheet_io::progresso::fase(feathersheet_io::progresso::Fase::Lendo);
            return Ok((feathersheet_io::parquet_read::ler_parquet_arquivo(path)?, 0));
        }
        if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("fshz")) {
            let lida = feathersheet_io::fshz_read::ler_fshz_arquivo(path)?;
            let folha = lida.a as usize;
            feathersheet_io::progresso::fase(feathersheet_io::progresso::Fase::Montando);
            return Ok((feathersheet_io::para_workbook::para_workbook_soltando(lida), folha));
        }
        if path.extension().and_then(|e| e.to_str()).is_some_and(|e| e.eq_ignore_ascii_case("xlsx") || e.eq_ignore_ascii_case("xlsm")) {
            // O leitor do Excel não conta bytes: barra sem porcentagem.
            feathersheet_io::progresso::fase(feathersheet_io::progresso::Fase::Montando);
            return feathersheet_io::xlsx_read::ler_xlsx_arquivo(path);
        }
        let bytes = std::fs::read(path)
            .map_err(|e| trf!("Não consegui ler {}: {e}", path.display()))?;
        Self::decodificar_com_separador(path, bytes, separador)
    }

    // Mantida para abertura por bytes e testes de round-trip. Em variantes de
    // carregamento incremental direto do disco, este helper pode não ser chamado.
    #[allow(dead_code)]
    pub(super) fn decodificar(path: &Path, bytes: Vec<u8>) -> Result<(Workbook, usize), String> {
        Self::decodificar_com_separador(path, bytes, None)
    }

    pub(super) fn decodificar_com_separador(path: &Path, bytes: Vec<u8>, separador: Option<char>) -> Result<(Workbook, usize), String> {
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        let (workbook, folha) = if ext == "fshz" {
            let lida = feathersheet_io::fshz_read::ler_fshz(&bytes)?;
            let folha = lida.a as usize;
            (feathersheet_io::para_workbook::para_workbook_soltando(lida), folha)
        } else if ext == "xlsx" || ext == "xlsm" {
            // OOXML incremental direto no armazenamento Rust, sem JSON de células.
            feathersheet_io::xlsx_read::ler_xlsx_workbook(bytes)?
        } else if ext == "csv" || ext == "tsv" {
            // APP-011: o adaptador do IO importa em lotes pelo parser
            // compartilhado, mas mantém o armazenamento colunar do Sheet.
            // XLSX e FSHZ acima continuam nos codecs nativos existentes.
            (feathersheet_io::csv_workbook::importar_csv_tsv_com_separador(bytes, ext == "tsv", separador)?, 0)
        } else {
            return Err(tr!("Formato não suportado. Abra .fshz, .xlsx, .xlsm, .csv, .tsv ou .parquet.").into());
        };
        Ok((workbook, folha))
    }

    pub(super) fn instalar(&mut self, path: PathBuf, workbook: Workbook, folha: usize) {
        self.perfil_geracao = self.perfil_geracao.wrapping_add(1);
        self.perfil_coluna = None;
        self.estatisticas_geracao = self.estatisticas_geracao.wrapping_add(1);
        self.estatisticas = None;
        self.tabela_editando = None;
        self.tabela_area = None;
        let ext = path.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        self.workbook = workbook;
        self.vinculos_novo_documento();
        self.vinculos_pedir_permissao();
        self.recarregar_macros();
        self.inspecoes.clear();
        self.eixos_grade.borrow_mut().limpar();
        self.congelados.clear();
        self.grade_oculta.clear();
        if ext == "xlsx" || ext == "xlsm" { self.aplicar_vistas_excel(&path); }
        self.congelar_menu_aberto = false;
        self.imagem_selecionada = None;
        self.arraste_imagem = None;
        self.limpar_cache_imagens();
        self.efeitos_imagem = efeitos_imagem::Estado::default();
        self.condicional.borrow_mut().limpar();
        self.folha = folha.min(self.workbook.len().saturating_sub(1));
        // Uma cópia de recuperação é uma nova planilha NÃO salva: nunca
        // sobrescrever automaticamente o snapshot nem colocá-lo em Recentes.
        let recuperacao = path.starts_with(Self::pasta_temporarios());
        self.caminho = if ext == "fshz" && !recuperacao { Some(path.clone()) } else { None };
        self.origem = (ext != "fshz" && !recuperacao).then(|| path.clone());
        if recuperacao {
            // Planilha recuperada: o nome do documento original volta.
            if let Some(origem) = self.recuperacao.origem_pendente.take() { self.origem = origem; }
        }
        self.alterado = ext != "fshz" || recuperacao;
        self.linha_inicio = 0;
        self.coluna_inicio = 0;
        self.trocar_selecao(0,0);
        self.editando_nome_aba = false;
        self.nome_aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
        if recuperacao {
            self.status = tr!("Cópia temporária recuperada. Use Salvar para criar o arquivo definitivo.").into();
        } else {
            self.status = trf!("Aberto: {}", path.display());
            self.registrar_arquivo_recente(&path);
        }
        if ext == "xlsm" { self.ler_vba_do_xlsm(&path); } else { self.vba_pendente = None; }
        self.bloquear_macros_do_arquivo();
        super::objetos::esquecer_rasteres();
        self.adiantar_graficos();
        // A pasta lida na outra thread acabou de entrar: o que sobrou da leitura volta ao sistema.
        feathersheet_core::paralelismo::devolver_memoria_ao_sistema();
    }

    /// Os gráficos da aba à vista viram imagem numa thread à parte (ao abrir e ao
    /// trocar de aba). Só os dela: os 7 gráficos de um relatório de 9 abas eram 13 MB
    /// de imagem para abas que ninguém abriu.
    // shortcut: mudar o zoom ou editar o gráfico rasteriza de novo na janela.
    pub(super) fn adiantar_graficos(&self) {
        let zoom = (f32::from(self.config.zoom) / 100.0).max(0.05);
        let graficos = self.graficos_da_aba(self.folha).into_iter()
            .map(|(_, ret, svg)| (ret, svg))
            .collect();
        super::objetos::adiantar_rasteres(graficos, zoom);
    }

    /// Linhas de grade e painéis congelados de cada aba, como o Excel
    /// mostrava (`<sheetView>`). Só estado da janela: o arquivo não muda.
    fn aplicar_vistas_excel(&mut self, path: &Path) {
        for (i, v) in feathersheet_io::xlsx_read::ler_vistas_arquivo(path).into_iter().enumerate() {
            let Some(id) = self.workbook.aba(i).map(|a| a.id) else { break };
            if !v.linhas_grade { self.grade_oculta.insert(id); }
            let (linhas, colunas) = v.congelar;
            if (linhas > 0 || colunas > 0) && linhas <= 30 && colunas <= 16 {
                self.congelados.insert(id, (linhas, colunas));
            }
        }
    }

    /// Helper para testes de round-trip. A GUI carrega arquivos via decodificar+instalar
    /// na tarefa assíncrona; não manter uma segunda rota de I/O no binário final.
    #[cfg(test)]
    pub(super) fn carregar(&mut self, path: PathBuf, bytes: Vec<u8>) -> Result<(), String> {
        let (workbook, folha) = Self::decodificar(&path, bytes)?;
        self.instalar(path, workbook, folha);
        Ok(())
    }
    /// Salva o .fshz sem travar a janela: a fotografia da pasta (`da_workbook`,
    /// ~0,4 s em 550 mil linhas × 83 colunas) é tirada aqui, e a compactação e
    /// a escrita (~8 s nesse arquivo) rodam em segundo plano. Antes, tudo rodava
    /// no evento e o sistema mostrava "não está respondendo".
    pub(super) fn gravar(&mut self, path: PathBuf) -> Task<Msg> {
        self.gravar_em_segundo_plano(path, false)
    }

    pub(super) fn gravar_em_segundo_plano(&mut self, path: PathBuf, copia: bool) -> Task<Msg> {
        self.confirmar();
        self.carimbar_propriedades();
        if self.salvando {
            self.status = tr!("Aguarde: o salvamento anterior ainda está em andamento.").into();
            return Task::none();
        }
        let pasta = feathersheet_io::da_workbook::da_workbook(&self.workbook, self.folha as u32);
        self.salvando = true;
        if !copia {
            // O que estava esperando o "Salvar" do aviso roda quando a gravação terminar.
            if let Some(p) = self.pendente.take() { self.apos_gravar = Some(p); }
            // Editar durante a gravação volta a marcar como alterado.
            self.alterado = false;
        }
        self.status = tr!("Salvando…").into();
        let destino = path.clone();
        Task::perform(async move {
            let inicio = Instant::now();
            let resultado = tokio::task::spawn_blocking(move || {
                feathersheet_io::arquivo::gravar_atomico_com(&destino, |arquivo| {
                    feathersheet_io::fshz_write::gravar_fshz_para(arquivo, &pasta)
                        .map_err(std::io::Error::other)
                }).map_err(|e| e.to_string())
            }).await.unwrap_or_else(|e| Err(trf!("Falha na tarefa de gravação: {e}")));
            resultado.map(|()| inicio.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
        }, move |resultado| Msg::Gravado(path.clone(), copia, resultado))
    }

    /// Testes: termina na hora a gravação que `gravar_em_segundo_plano`
    /// deixou em `Task` (o runtime do Iced não roda nos testes), pelo mesmo
    /// gravador atômico e pela mesma conclusão do app.
    #[cfg(test)]
    pub(super) fn concluir_gravacao_teste(&mut self, path: PathBuf, copia: bool) {
        assert!(self.salvando, "nenhuma gravação foi iniciada");
        let pasta = feathersheet_io::da_workbook::da_workbook(&self.workbook, self.folha as u32);
        let resultado = feathersheet_io::arquivo::gravar_atomico_com(&path, |arquivo| {
            feathersheet_io::fshz_write::gravar_fshz_para(arquivo, &pasta)
                .map_err(std::io::Error::other)
        }).map(|()| 1).map_err(|e| e.to_string());
        let _ = self.gravacao_terminou(path, copia, resultado);
    }

    pub(super) fn gravacao_terminou(&mut self, path: PathBuf, copia: bool, resultado: Result<u64, String>) -> Task<Msg> {
        self.salvando = false;
        match resultado {
            Ok(ms) if copia => {
                self.registrar_arquivo_recente(&path);
                self.status = trf!("Cópia salva em: {}", path.display());
                let _ = ms;
            }
            Ok(ms) => {
                self.auto_salvar_ultimo_custo_ms = ms;
                self.caminho = Some(path.clone());
                self.registrar_arquivo_recente(&path);
                let tamanho = std::fs::metadata(&path).map(|m| m.len()).unwrap_or(0);
                self.status = trf!("Salvo: {:.1} MB em {:.1} s", tamanho as f64 / 1_048_576.0, ms as f64 / 1000.0);
                if let Some(p) = self.apos_gravar.take() { return self.executar_pendente(p); }
            }
            Err(e) => {
                if !copia { self.alterado = true; }
                self.apos_gravar = None;
                self.status = trf!("Erro ao salvar: {e}");
            }
        }
        Task::none()
    }

    /// Diretório persistente (e acessível pelo gerenciador de arquivos).
    /// Não usa a pasta temporária do sistema, que pode ser apagada no reboot.
    pub(super) fn pasta_temporarios() -> PathBuf {
        dialogos::pasta_padrao().join("Temporarios")
    }

    pub(super) fn existem_temporarios() -> bool {
        std::fs::read_dir(Self::pasta_temporarios()).is_ok_and(|entradas| {
            entradas.filter_map(Result::ok).any(|item| {
                item.path().is_file()
                    && item.path().extension().and_then(|s| s.to_str()) == Some("fshz")
            })
        })
    }

    /// Salva um snapshot separado, sem alterar o arquivo original.
    /// Só avança com a ação pendente se a escrita atômica foi bem-sucedida.
    pub(super) fn guardar_temporariamente(&mut self) -> Task<Msg> {
        let Some(destino) = self.pendente else { return Task::none(); };
        if !self.confirmar() {
            self.status = tr!("Conclua a edição antes de guardar temporariamente.").into();
            return Task::none();
        }
        let pasta = Self::pasta_temporarios();
        if let Err(e) = std::fs::create_dir_all(&pasta) {
            self.status = trf!("Não foi possível criar a pasta de recuperação: {e}");
            return Task::none();
        }
        let nome = self.caminho.as_ref()
            .and_then(|path| path.file_stem()).and_then(|s| s.to_str())
            .unwrap_or("Sem_titulo");
        let nome: String = nome.chars().take(48)
            .map(|c| if c.is_alphanumeric() || c == '-' || c == '_' { c } else { '_' })
            .collect();
        let instante = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_millis();
        let mut caminho = pasta.join(format!("{nome}-{instante}-{}.fshz", std::process::id()));
        let mut sufixo = 1u32;
        while caminho.exists() {
            caminho = pasta.join(format!("{nome}-{instante}-{}-{sufixo}.fshz", std::process::id()));
            sufixo += 1;
        }
        match feathersheet_io::da_workbook::pasta_gravar_arquivo(
            &self.workbook, self.folha as u32, &caminho
        ) {
            Ok(()) => {
                // O arquivo original continua sem salvar: se o seletor para
                // abrir/recuperar for cancelado, o documento ainda é "alterado".
                self.executar_pendente(destino)
            }
            Err(erro) => {
                self.status = trf!("Não foi possível guardar a cópia temporária: {erro}");
                Task::none()
            }
        }
    }

    /// Backups manuais usam o .fshz já salvo: sem clonar Workbook nem
    /// apagar o documento em edição. O fluxo funciona mesmo com arquivos grandes.
    pub(super) fn criar_backup(&mut self) -> Task<Msg> {
        if self.carregando || self.pendente.is_some() ||
            self.assistente_divisao.as_ref().is_some_and(|e| e.conversao_em_andamento())
        {
            self.status = tr!("Aguarde a operação atual antes de criar um backup.").into();
            return Task::none();
        }
        let Some(origem) = self.caminho.clone() else {
            self.status = tr!("Salve a planilha como .fshz antes de criar um backup.").into();
            return Task::none();
        };
        if self.alterado || self.editor_formula.ativo {
            self.status = tr!("Salve as alterações antes do backup para não copiar uma versão antiga.").into();
            return Task::none();
        }
        let pasta = Self::pasta_temporarios();
        self.status = tr!("Criando cópia de segurança do último arquivo salvo…").into();
        Task::perform(async move {
            match tokio::task::spawn_blocking(move ||
                feathersheet_io::backup_snapshot::criar_backup(&origem, &pasta)
            ).await {
                Ok(resultado) => resultado,
                Err(e) => Err(trf!("Falha na tarefa de cópia: {e}")),
            }
        }, Msg::BackupCriado)
    }

    pub(super) fn dialogo_recuperar_temporario(&self) -> Task<Msg> {
        let pasta = Self::pasta_temporarios();
        Task::perform(async move {
            rfd::AsyncFileDialog::new().set_directory(pasta)
                .add_filter(tr!("Cópias de segurança FeatherSheet"), &["fshz"])
                .pick_file().await.map(|file| file.path().to_path_buf())
        }, Msg::DestinoTemporario)
    }
    pub(super) fn executar_pendente(&mut self, tipo: Pendente) -> Task<Msg> {
        self.pendente = None;
        // Salvou ou descartou: a cópia de recuperação do documento atual sai.
        self.descartar_recuperacao();
        match tipo {
            Pendente::Novo => {
                self.em_tela_inicio = false;
                self.perfil_geracao = self.perfil_geracao.wrapping_add(1);
                self.perfil_coluna = None;
                self.estatisticas_geracao = self.estatisticas_geracao.wrapping_add(1);
                self.estatisticas = None;
                self.workbook = Self::pasta_nova();
                self.vinculos_novo_documento();
                self.recarregar_macros();
                (self.macros_bloqueadas, self.aviso_macros) = (false, false);
                self.inspecoes.clear();
                self.eixos_grade.borrow_mut().limpar();
                // O cache das regras é por id + versão da aba; uma pasta nova
                // repete os mesmos números e herdaria as regras da anterior.
                self.condicional.borrow_mut().limpar();
                self.congelados.clear();
                self.grade_oculta.clear();
                self.congelar_menu_aberto = false;
                self.imagem_selecionada = None;
                self.arraste_imagem = None;
                self.limpar_cache_imagens();
                self.efeitos_imagem.aberto = false;
                self.caminho = None;
                self.origem = None;
                self.alterado = false;
                self.folha = 0; self.linha_inicio = 0; self.coluna_inicio = 0;
                self.ancorar_rodape = false;
                self.trocar_selecao(0, 0);
                self.editando_nome_aba = false;
                self.nome_aba = self.workbook.nomes().first().cloned().unwrap_or_default();
                self.status = tr!("Nova pasta criada").into();
                Task::none()
            }
            Pendente::Abrir => {
                self.carregando = true;
                self.status = tr!("Aguardando seleção ou carregando planilha…").into();
                self.dialogo_abrir()
            },
            Pendente::Recente => {
                if self.carregando {
                    self.status = tr!("Aguarde a planilha em carregamento.").into();
                    return Task::none();
                }
                let Some(path) = self.recente_destino.take() else {
                    self.status = tr!("Nenhum arquivo recente selecionado.").into();
                    return Task::none();
                };
                if !path.is_file() {
                    self.status = trf!("Arquivo recente não encontrado: {}", path.display());
                    return Task::none();
                }
                self.em_tela_inicio = false;
                self.menu_aberto = false;
                self.carregando = true;
                self.status = trf!("Abrindo recente: {}", path.display());
                Self::tarefa_abrir_recente(path, self.config.separador_csv())
            }
            Pendente::Modelo(id) => match super::menu_principal::modelos::criar(id) {
                Some(modelo) => {
                    // Mesmo fluxo de Novo (zera estado da pasta); depois entra o conteúdo do modelo.
                    let _ = self.executar_pendente(Pendente::Novo);
                    self.workbook = modelo;
                    self.vinculos_novo_documento();
                    self.recarregar_macros();
                    for &(aba, linhas, colunas) in super::menu_principal::modelos::congelamento(id) {
                        if let Some(a) = self.workbook.aba(aba) { self.congelados.insert(a.id, (linhas, colunas)); }
                    }
                    self.eixos_grade.borrow_mut().limpar();
                    self.condicional.borrow_mut().limpar();
                    self.status = tr!("Nova planilha criada a partir de modelo").into();
                    Task::none()
                }
                None => {
                    self.status = trf!("Modelo indisponível: {id}");
                    Task::none()
                }
            },
            Pendente::RecuperarTemporario => self.dialogo_recuperar_temporario(),
            Pendente::Sair => { mcp_vivo::encerrar(); vinculos::encerrar(); Task::window(pelicano::WindowCmd::Exit) }
        }
    }
    pub(super) fn proteger(&mut self, alvo: Pendente) -> Task<Msg> {
        self.confirmar();
        if self.salvando {
            // Fechar ou trocar de arquivo no meio da gravação perderia o salvamento.
            self.apos_gravar = Some(alvo);
            self.status = tr!("Terminando de salvar…").into();
            return Task::none();
        }
        if self.alterado {
            self.pendente = Some(alvo);
            Task::none()
        } else { self.executar_pendente(alvo) }
    }
    /// Só o diálogo: a leitura começa em `Msg::ArquivoEscolhido`, depois de
    /// sair da tela inicial, para a barra de progresso do rodapé aparecer.
    pub(super) fn dialogo_abrir(&self) -> Task<Msg> {
        Task::perform(async move {
            // Um filtro por tipo, como o Excel; o primeiro é o que vem escolhido.
            // O .xls (binário do Excel 97–2003) fica de fora até haver leitor.
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                .add_filter(tr!("Todas as planilhas"), &["fshz", "xlsx", "xlsm", "csv", "tsv", "parquet"])
                .add_filter(tr!("Pasta de trabalho do FeatherSheet (.fshz)"), &["fshz"])
                .add_filter(tr!("Pasta de trabalho do Excel (.xlsx)"), &["xlsx"])
                .add_filter(tr!("Pasta de trabalho do Excel com macros (.xlsm)"), &["xlsm"])
                .add_filter(tr!("CSV (.csv)"), &["csv"])
                .add_filter(tr!("Texto separado por tabulações (.tsv)"), &["tsv"])
                .add_filter("Parquet (.parquet)", &["parquet"])
                .pick_file().await.map(|file| file.path().to_path_buf())
        }, Msg::ArquivoEscolhido)
    }
    pub(super) fn tarefa_abrir_recente(path: PathBuf, separador: Option<char>) -> Task<Msg> {
        // Usar a preferência persistida evita alterar os despachos antigos.
        let limite_gb = Configuracao::carregar().limite_memoria_gb;
        Task::perform(async move {
            match tokio::task::spawn_blocking(move || {
                let inicio = Instant::now();
                feathersheet_io::progresso::comecar(&path);
                limite_memoria::verificar(limite_gb)?;
                let (workbook, folha) = Self::decodificar_caminho(&path, separador)?;
                limite_memoria::verificar(limite_gb)?;
                Ok::<PastaLida, String>(PastaLida {
                    caminho: path,
                    dados: Arc::new(Mutex::new(Some((workbook, folha)))),
                    ms: inicio.elapsed().as_millis().min(u128::from(u64::MAX)) as u64,
                })
            }).await {
                Ok(resultado) => resultado.map(Some),
                Err(e) => Err(trf!("Falha ao carregar arquivo: {e}")),
            }
        }, Msg::Aberto)
    }
    pub(super) fn dialogo_salvar_copia(&self) -> Task<Msg> {
        let base = self.caminho.as_ref()
            .and_then(|p| p.file_stem()).and_then(|s| s.to_str())
            .unwrap_or(tr!("Sem título"));
        let nome = trf!("Cópia de {base}.fshz");
        Task::perform(async move {
            rfd::AsyncFileDialog::new()
                .set_directory(dialogos::pasta_padrao())
                .add_filter("FeatherSheet", &["fshz"])
                .set_file_name(&nome)
                .save_file().await.map(|f| f.path().to_path_buf())
        }, Msg::DestinoCopia)
    }

    /// Grava uma segunda cópia sem trocar o documento ativo e sem descartar
    /// a marca de alterações pendentes na planilha original.
    pub(super) fn salvar_copia(&mut self, path: PathBuf) -> Task<Msg> {
        self.gravar_em_segundo_plano(path, true)
    }

    pub(super) fn dialogo_salvar(&self) -> Task<Msg> {
        let nome = format!("{}.fshz", self.nome_base_documento());
        Task::perform(async move {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao()).add_filter("FeatherSheet", &["fshz"])
                .set_file_name(&nome)
                .save_file().await.map(|f| f.path().to_path_buf())
        }, Msg::DestinoSalvar)
    }

    /// Arquivo > Renomear…: o diálogo de salvar escolhe o novo caminho.
    /// Sem arquivo ainda (planilha nova), comporta-se como Salvar como.
    pub(super) fn dialogo_renomear(&self) -> Task<Msg> {
        let Some(atual) = self.caminho.clone() else { return self.dialogo_salvar() };
        let pasta = atual.parent().map(PathBuf::from).unwrap_or_else(dialogos::pasta_padrao);
        let nome = atual.file_name().map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| tr!("Sem título.fshz").into());
        Task::perform(async move {
            rfd::AsyncFileDialog::new().set_directory(pasta).add_filter("FeatherSheet", &["fshz"])
                .set_file_name(&nome)
                .save_file().await.map(|f| f.path().to_path_buf())
        }, Msg::DestinoRenomear)
    }

    /// Renomeia no disco o arquivo aberto e aponta o documento para o novo
    /// caminho. Se há alterações pendentes, grava antes, como o TypeScript.
    pub(super) fn renomear_arquivo_atual(&mut self, mut novo: PathBuf) {
        self.confirmar();
        let Some(antigo) = self.caminho.clone() else {
            self.status = tr!("Salve a planilha antes de renomear").into();
            return;
        };
        if novo.extension().and_then(|s| s.to_str()) != Some("fshz") {
            novo.set_extension("fshz");
        }
        if novo == antigo { return; }
        if novo.exists() {
            self.status = trf!("Já existe um arquivo com o nome {}", novo.display());
            return;
        }
        if self.alterado {
            if let Err(e) = feathersheet_io::da_workbook::pasta_gravar_arquivo(
                &self.workbook, self.folha as u32, &antigo
            ) {
                self.status = trf!("Erro ao salvar antes de renomear: {e}");
                return;
            }
            self.alterado = false;
        }
        match std::fs::rename(&antigo, &novo) {
            Ok(()) => {
                let antigo_txt = antigo.to_string_lossy().into_owned();
                if let Some(i) = self.recentes.arquivos.iter().position(|a| a.caminho == antigo_txt) {
                    self.recentes.esquecer(i);
                }
                self.caminho = Some(novo.clone());
                self.registrar_arquivo_recente(&novo);
                let nome = novo.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default();
                self.status = trf!("Arquivo renomeado para {nome}");
            }
            Err(e) => self.status = trf!("Erro ao renomear: {e}"),
        }
    }

    pub(super) fn dialogo_exportar_xlsx(&self) -> Task<Msg> {
        let nome = format!("{}.xlsx", self.nome_base_documento());
        Task::perform(async move {
            rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                .add_filter(tr!("Pasta do Excel"), &["xlsx"])
                .set_file_name(&nome)
                .save_file().await.map(|f| f.path().to_path_buf())
        }, Msg::DestinoXlsx)
    }


    /// Escolhe o arquivo antes de gerar os dados — cancelar não dispara IO.
    /// true = TSV e false = CSV; exporta apenas a aba atual.
    pub(super) fn dialogo_exportar_delimitado(&self, tsv: bool) -> Task<Msg> {
        // O nome do documento (como o Excel); com mais de uma aba, a aba
        // exportada vai junto. Antes sugeria só "Planilha1".
        let documento = self.nome_base_documento();
        let nome = match self.workbook.aba(self.folha) {
            Some(aba) if self.workbook.len() > 1 => format!("{documento} - {}", aba.sheet.name),
            _ => documento,
        };
        // Um nome de aba pode conter barras; não usá-las como diretórios.
        let nome: String = nome.chars().map(|c|
            if "/\\:*?\"<>|".contains(c) { '_' } else { c }
        ).collect();
        let ext = if tsv { "tsv" } else { "csv" };
        Task::perform(async move {
            let dialogo = rfd::AsyncFileDialog::new()
                .set_directory(dialogos::pasta_padrao())
                .set_file_name(&format!("{nome}.{ext}"));
            let dialogo = if tsv {
                dialogo.add_filter(tr!("Tabulação"), &["tsv"])
            } else {
                dialogo.add_filter("CSV", &["csv"])
            };
            dialogo.save_file().await.map(|arquivo| arquivo.path().to_path_buf())
        }, move |destino| Msg::DestinoDelimitado(destino, tsv))
    }

    pub(super) fn gravar_delimitado(&mut self, path: PathBuf, tsv: bool) -> Task<Msg> {
        // Exportação é uma cópia, sem mudar .fshz, undo ou marcador de alteração.
        self.exportar_em_segundo_plano(path, Exportacao::Delimitado(tsv))
    }

    /// Exporta fora da thread da janela (#100): com 1 milhão de linhas, gerar
    /// o .xlsx aqui deixava o app "não está respondendo". Aqui só sai o retrato
    /// do .fshz (o mesmo do Salvar, ~50 ms); a outra thread remonta a pasta a
    /// partir dele e grava. Custa a RAM de uma segunda pasta enquanto exporta.
    pub(super) fn exportar_em_segundo_plano(&mut self, path: PathBuf, tipo: Exportacao) -> Task<Msg> {
        self.confirmar();
        if matches!(tipo, Exportacao::Xlsx) { self.carimbar_propriedades(); }
        if self.salvando {
            self.status = tr!("Aguarde: o salvamento anterior ainda está em andamento.").into();
            return Task::none();
        }
        let folha = self.folha;
        let pasta = feathersheet_io::da_workbook::da_workbook(&self.workbook, folha as u32);
        self.salvando = true;
        self.status = tr!("Exportando…").into();
        Task::perform(async move {
            tokio::task::spawn_blocking(move || exportar_retrato(&pasta, folha, &path, tipo))
                .await.unwrap_or_else(|e| Err(trf!("Falha na tarefa de exportação: {e}")))
        }, Msg::Exportado)
    }

    pub(super) fn exportacao_terminou(&mut self, resultado: Result<(String, std::collections::BTreeSet<String>), String>) {
        self.salvando = false;
        match resultado {
            Ok((aviso, usadas)) => {
                self.status = aviso;
                self.avisar_funcoes_fora_do_excel(&usadas);
            }
            Err(e) => self.status = e,
        }
    }

    /// Pergunta o destino antes de percorrer as células; cancelar não faz IO.
    pub(super) fn dialogo_exportar_json(&self) -> Task<Msg> {
        let nome = self.caminho.as_deref()
            .and_then(|p| p.file_stem()).and_then(|s| s.to_str())
            .unwrap_or(tr!("Planilha")).to_owned();
        Task::perform(async move {
            rfd::AsyncFileDialog::new()
                .set_directory(dialogos::pasta_padrao())
                .add_filter("JSON", &["json"])
                .set_file_name(&format!("{nome}.json"))
                .save_file().await.map(|f| f.path().to_path_buf())
        }, Msg::DestinoJson)
    }

    /// Exporta todas as abas sem materializar Workbook::toJSON na RAM.
    /// A operação gera uma cópia e não altera a associação do .fshz atual.
    pub(super) fn gravar_json(&mut self, path: PathBuf) -> Task<Msg> {
        self.exportar_em_segundo_plano(path, Exportacao::Json)
    }

    pub(super) fn gravar_xlsx(&mut self, path: PathBuf) -> Task<Msg> {
        // Exportar para Excel não altera o arquivo .fshz de trabalho.
        self.exportar_em_segundo_plano(path, Exportacao::Xlsx)
    }

}

#[derive(Clone, Copy)]
pub(super) enum Exportacao { Xlsx, Json, Delimitado(bool) }

/// A exportação em si, na thread de trabalho: remonta a pasta do retrato e grava.
fn exportar_retrato(pasta: &feathersheet_io::fshz_write::PastaBruta, folha: usize, path: &std::path::Path, tipo: Exportacao)
    -> Result<(String, std::collections::BTreeSet<String>), String> {
    let bytes = feathersheet_io::fshz_write::gerar_fshz2(pasta)?;
    let wb = feathersheet_io::para_workbook::para_workbook_soltando(feathersheet_io::fshz_read::ler_fshz(&bytes)?);
    drop(bytes);
    match tipo {
        Exportacao::Xlsx => {
            let (bytes, usadas) = feathersheet_io::xlsx_write::gerar_xlsx_com_funcoes(&wb)
                .map_err(|e| trf!("Erro ao gerar Excel: {e}"))?;
            feathersheet_io::arquivo::gravar_atomico(path, &bytes).map_err(|e| trf!("Erro ao gravar Excel: {e}"))?;
            Ok((trf!("Excel exportado: {}", path.display()), usadas))
        }
        Exportacao::Json => feathersheet_io::json_workbook::gravar_json(&wb, folha, path)
            .map(|()| (trf!("JSON exportado: {}", path.display()), Default::default()))
            .map_err(|e| trf!("Erro ao exportar JSON: {e}")),
        Exportacao::Delimitado(tsv) => feathersheet_io::csv_workbook::gravar_csv_tsv(&wb, folha, path, tsv)
            .map(|()| (trf!("{} exportado: {}", if tsv { "TSV" } else { "CSV" }, path.display()), Default::default()))
            .map_err(|e| trf!("Erro ao exportar {}: {e}", if tsv { "TSV" } else { "CSV" })),
    }
}

/// O destino do Salvar com `.fshz` no fim: acrescenta, não troca ("Relatório 2026.10"
/// vira "Relatório 2026.10.fshz", não "Relatório 2026.fshz"). A janela de arquivo só
/// perguntou sobre o nome digitado: se o nome final é outro arquivo que já existe, recusa.
pub(super) fn com_extensao_fshz(path: PathBuf) -> Result<PathBuf, String> {
    if path.extension().is_some_and(|e| e.eq_ignore_ascii_case("fshz")) { return Ok(path); }
    let mut nome = path.into_os_string();
    nome.push(".fshz");
    let destino = PathBuf::from(nome);
    if destino.exists() { return Err(trf!("Já existe um arquivo com o nome {}", destino.display())); }
    Ok(destino)
}

#[cfg(test)]
mod testes_exportar {
    use super::*;

    #[test]
    fn exportar_pelo_retrato_grava_os_valores_calculados() {
        let mut wb = feathersheet_core::planilha::workbook::Workbook::new();
        wb.criar_aba("Dados");
        let aba = wb.aba_mut(0).unwrap();
        aba.sheet.set_cell(0, 0, "10");
        aba.sheet.set_cell(1, 0, "=A1*2");
        let pasta = feathersheet_io::da_workbook::da_workbook(&wb, 0);
        let dir = std::env::temp_dir().join(format!("fs-exportar-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let csv = dir.join("a.csv");
        let (aviso, _) = exportar_retrato(&pasta, 0, &csv, Exportacao::Delimitado(false)).unwrap();
        assert!(aviso.contains("CSV"));
        let texto = std::fs::read_to_string(&csv).unwrap();
        assert!(texto.contains("10") && texto.contains("20"), "{texto}");
        let xlsx = dir.join("a.xlsx");
        exportar_retrato(&pasta, 0, &xlsx, Exportacao::Xlsx).unwrap();
        let (lida, _) = feathersheet_io::xlsx_read::ler_xlsx_workbook(std::fs::read(&xlsx).unwrap()).unwrap();
        assert_eq!(lida.aba(0).unwrap().sheet.get_raw(1, 0), "=A1*2");
        let _ = std::fs::remove_dir_all(&dir);
    }
}
