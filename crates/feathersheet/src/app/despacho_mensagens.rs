//! Despacho de mensagens do aplicativo.
//! Extração mecânica: preservar semântica do match até dividir por domínios.
use super::*;

impl App {
    pub(super) fn processar_mensagem(&mut self, msg: Msg) -> Task<Msg> {
        // Durante a transação incremental, nenhum exportador/salvamento
        // pode serializar a aba parcialmente modificada.
        if self.assistente_divisao.as_ref()
            .is_some_and(|e| e.conversao_em_andamento())
            && matches!(&msg,
                Msg::Salvar | Msg::SalvarComo | Msg::MenuSalvarCopia
                | Msg::DestinoSalvar(_) | Msg::DestinoCopia(_)
                | Msg::RenomearArquivo | Msg::DestinoRenomear(_)
                | Msg::ExportarXlsx | Msg::DestinoXlsx(_)
                | Msg::ExportarJson | Msg::DestinoJson(_)
                | Msg::ExportarDelimitado(_) | Msg::DestinoDelimitado(..))
        {
            self.status=tr!("Aguarde a conversão terminar ou cancele antes de salvar").into();
            return Task::none();
        }
        match msg {
            Msg::OpenQuery(evento) => self.evento_openquery_janela(evento),
            Msg::Macros => self.alternar(Auxiliar::Macros),
            Msg::OcultarDimensoes(coluna) => {
                self.alterar_visibilidade_eixos(coluna, true);
                Task::none()
            }
            Msg::ReexibirDimensoes(coluna) => {
                self.alterar_visibilidade_eixos(coluna, false);
                Task::none()
            }
            // Movimento do mouse nas janelas auxiliares (Configurações, Ajuda, Macros…) não
            // mexe na planilha: antes virava posição da grade e refazia a tela principal a
            // cada movimento, deixando tudo lento com outra janela aberta.
            Msg::Atualizacao(e) => self.evento_atualizacao(e),
            Msg::CursorJanela(id, pos) => {
                if self.janelas.principal == Some(id) {
                    self.update(Msg::Grafico(graficos::Evento::Cursor(pos)))
                } else {
                    Task::none()
                }
            }
            Msg::Grafico(e) => {
                if let graficos::Evento::Cursor(pos) = &e {
                    self.ponteiro_x = pos.x;
                    self.ponteiro_y = pos.y;
                }
                // A subscription já recebe cursor e mouse-up globalmente.
                // Durante gesto de imagem, não movimentar gráficos também.
                match e {
                    graficos::Evento::Cursor(pos) if self.arraste_imagem.is_some() => {
                        self.mover_cursor_imagem(pos);
                        self.graficos.cursor = pos;
                        Task::none()
                    }
                    graficos::Evento::Soltar if self.arraste_imagem.is_some() => {
                        self.soltar_imagem();
                        Task::none()
                    }
                    graficos::Evento::Cursor(pos) if self.arraste_botao_macro.is_some() => {
                        self.mover_cursor_botao_macro(pos);
                        self.graficos.cursor = pos;
                        Task::none()
                    }
                    graficos::Evento::Soltar if self.arraste_botao_macro.is_some() => {
                        self.soltar_botao_macro();
                        Task::none()
                    }
                    outro => self.evento_grafico(outro),
                }
            }
            Msg::AlternarMenuTabelas => self.alternar_menu_estilos(true),
            Msg::AlternarMenuEstilos => self.alternar_menu_estilos(false),
            Msg::FecharMenusEstilos => {
                self.tabelas_menu_aberto = false;
                self.estilos_menu_aberto = false;
                Task::none()
            }
            Msg::TabelaCriarPeloMenu => {
                self.tabelas_menu_aberto = false;
                self.tabela_estilo_pendente = None;
                self.update(Msg::TabelaAbrir)
            }
            Msg::TabelaEstiloEscolher(tema) => {
                self.tabelas_menu_aberto = false;
                if let Some(tabela) =
                    tabelas::sob_celula(&self.workbook,self.folha,self.linha,self.coluna)
                {
                    // A galeria funciona mesmo quando o usuário está na aba Início,
                    // sem exigir que a Ribbon contextual tenha sido aberta antes.
                    self.tabela_editando = tabela["id"].as_str().map(str::to_owned);
                    self.acao_tabela(tabelas::Acao::Tema(tema))
                } else {
                    self.tabela_estilo_pendente = Some(tema);
                    self.update(Msg::TabelaAbrir)
                }
            }
            Msg::EstiloCelulaAplicar(id) => {
                self.estilos_menu_aberto = false;
                self.aplicar_preset_celula(id);
                Task::none()
            }
            Msg::TabelaAbrir => {
                self.confirmar();
                self.menu_aberto = false;
                self.contexto = None;
                let selecao = self.area_selecionada();
                let inicio = std::time::Instant::now();
                match tabelas::sugerir(&self.workbook, self.folha, selecao)
                    .and_then(|(area, cab, branca)| Ok((
                        area, cab, branca, tabelas::nome_sugerido(&self.workbook)?
                    ))) {
                    Ok((area, cab, branca, nome)) => {
                        self.monitor.registrar(Nivel::Info, tr!("Tabela"), &trf!(
                            "área sugerida {}:{} · {:.1} ms", cell_name(area.r0, area.c0),
                            cell_name(area.r1, area.c1), inicio.elapsed().as_secs_f64() * 1000.0));
                        self.tabela_area = Some(area);
                        self.tabela_cabecalho = cab;
                        self.tabela_branca = branca;
                        self.tabela_nome = nome;
                    }
                    Err(erro) => self.status = erro,
                }
                Task::none()
            }
            Msg::TabelaAcao(acao) => self.acao_tabela(acao),
            Msg::BarraStatus(evento) => self.evento_barra_status(evento),
            Msg::McpAtivo(ativo) => {
                self.config.mcp_ativo = ativo;
                self.config.gravar();
                self.mcp_estado = mcp_vivo::ligar(ativo);
                self.monitor.registrar(Nivel::Info, "MCP", &self.mcp_estado);
                Task::none()
            }
            Msg::McpSoLeitura(sim) => {
                self.config.mcp_so_leitura = sim;
                self.config.gravar();
                mcp_vivo::somente_leitura(sim);
                Task::none()
            }
            Msg::CopiarTexto(texto) => {
                self.status = tr!("Copiado para a área de transferência.").into();
                Task::write_clipboard(texto)
            }
            Msg::TabelaNome(nome) => { self.tabela_nome = nome; Task::none() }
            Msg::TabelaCabecalho(v) => { self.tabela_cabecalho = v; Task::none() }
            Msg::TabelaCancelar => {
                self.tabela_area = None;
                self.tabela_estilo_pendente = None;
                Task::none()
            }
            Msg::TabelaConfirmar => {
                if let Some(area) = self.tabela_area.clone() {
                    let inicio = std::time::Instant::now();
                    match tabelas::criar(&mut self.workbook, self.folha,
                        area.clone(), &self.tabela_nome, self.tabela_cabecalho, self.tabela_branca)
                    {
                        Ok(()) => {
                            self.monitor.registrar(Nivel::Info, tr!("Tabela"), &trf!(
                                "{} criada · {:.1} ms", self.tabela_nome,
                                inicio.elapsed().as_secs_f64() * 1000.0));
                            self.alterado = true;
                            self.status = trf!("Tabela {} criada em {}:{}",
                                self.tabela_nome, cell_name(area.r0,area.c0),
                                cell_name(area.r1,area.c1));
                            self.tabela_area = None;
                            self.linha = area.r0;
                            self.coluna = area.c0;
                            self.fim_selecao = (area.r1, area.c1);
                            self.tabela_editando = tabelas::sob_celula(
                                &self.workbook, self.folha, area.r0, area.c0)
                                .and_then(|t|t["id"].as_str().map(str::to_owned));
                            self.tabela_nome_novo = self.tabela_nome.clone();
                            self.tabela_renomeando = false;
                            self.faixa_ativa = AbaFaixa::Tabela;
                            self.tabela_popup_filtro = false;
                            if let Some(tema) = self.tabela_estilo_pendente.take() {
                                let _ = self.acao_tabela(tabelas::Acao::Tema(tema));
                            }
                        }
                        Err(erro) => { self.tabela_estilo_pendente = None; self.status = erro; },
                    }
                }
                Task::none()
            }
            Msg::ImagemCelula(evento) => self.evento_imagem_celula(evento),
            Msg::FormatarImagem(evento) => self.evento_formatar_imagem(evento),
            Msg::ImagemPressionar(id, gesto) => {
                self.pressionar_imagem(id, gesto);
                Task::none()
            },
            Msg::ImagemEfeitosAbrir => {
                let resultado = self.workbook.aba(self.folha)
                    .ok_or_else(|| tr!("A aba selecionada não existe.").to_string())
                    .and_then(|aba| imagens_importadas::ultima_imagem(&aba.sheet))
                    .and_then(|ultima| ultima
                        .ok_or_else(|| tr!("Insira uma imagem PNG/JPEG nesta aba antes de editar.").to_string()));
                match resultado {
                    Ok((id, dados, nome)) => match self.efeitos_imagem.carregar(&dados, &nome) {
                        Ok(()) => {
                            self.efeitos_imagem.imagem_id = Some(id);
                            self.status = trf!("Editando efeitos de imagem: {nome}");
                        }
                        Err(erro) => self.status = erro,
                    },
                    Err(erro) => self.status = erro,
                }
                Task::none()
            }
            Msg::ImagemLaboratorioAbrir => {
                // Inserir apenas: editor e presets visuais são etapas futuras.
                self.efeitos_imagem.aberto = false;
                Task::perform(async {
                    let Some(arquivo) = rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                        .add_filter(tr!("PNG e JPEG"), &["png", "jpg", "jpeg"])
                        .pick_file().await else { return Ok(None); };
                    let caminho = arquivo.path().to_path_buf();
                    // I/O pesado fora do loop Iced; limites antes da leitura.
                    match tokio::task::spawn_blocking(move || {
                        let nome = caminho.file_name().and_then(|s| s.to_str())
                            .unwrap_or(tr!("Imagem")).to_owned();
                        let tamanho = std::fs::metadata(&caminho)
                            .map_err(|e| trf!("Não consegui acessar a imagem: {e}"))?.len();
                        if tamanho == 0 || tamanho > imagens_importadas::MAX_BYTES as u64 {
                            return Err(tr!("Escolha um PNG/JPEG de até 16 MiB.").to_string());
                        }
                        let bytes = std::fs::read(&caminho)
                            .map_err(|e| trf!("Falha ao ler imagem: {e}"))?;
                        Ok::<_, String>((bytes, nome))
                    }).await {
                        Ok(Ok(imagem)) => Ok(Some(imagem)),
                        Ok(Err(e)) => Err(e),
                        Err(e) => Err(trf!("Falha ao carregar imagem: {e}")),
                    }
                }, Msg::ImagemLaboratorioCarregada)
            }
            Msg::ImagemLaboratorioCarregada(resultado) => {
                match resultado {
                    Ok(Some((bytes, nome))) => {
                        self.confirmar();
                        let (linha, coluna) = (self.linha, self.coluna);
                        let resultado = self.workbook.aba_mut(self.folha)
                            .ok_or_else(|| tr!("A aba selecionada não existe.").to_string())
                            .and_then(|aba| imagens_importadas::importar(
                                &mut aba.sheet, &bytes, &nome, linha, coluna,
                            ));
                        match resultado {
                            Ok(id) => {
                                self.alterado = true;
                                self.invalidar_cache_objetos();
                                // Após a importação, a própria imagem fica selecionada:
                                // é possível arrastá-la imediatamente e a alça aparece.
                                self.imagem_selecionada = self.workbook.aba(self.folha)
                                    .map(|aba| (aba.id, id.clone()));
                                self.arraste_imagem = None;
                                if let Some(imagem) = crate::compat::imagem_de_bytes(&bytes) {
                                    self.imagens_importadas.handles.borrow_mut().insert(id, imagem);
                                }
                                self.status = trf!("Imagem inserida na planilha: {nome}. Arraste para mover ou use as alças para dimensionar.");
                            }
                            Err(e) => self.status = e,
                        }
                    },
                    Ok(None) => self.status = tr!("Seleção de imagem cancelada.").into(),
                    Err(e) => self.status = e,
                }
                Task::none()
            }
            Msg::ImagemLaboratorioAplicar(chave) => {
                // Calcular os pixels sem alterar a pasta; só efetivar o
                // resultado se o PNG novo passar pelas validações do FSHZ.
                let resultado = self.efeitos_imagem.aplicar(chave)
                    .and_then(|_| self.efeitos_imagem.png_exportacao())
                    .and_then(|png| {
                        let id = self.efeitos_imagem.imagem_id.as_deref()
                            .ok_or_else(|| tr!("Nenhuma imagem da planilha selecionada.").to_string())?;
                        let aba = self.workbook.aba_mut(self.folha)
                            .ok_or_else(|| tr!("A aba selecionada não existe mais.").to_string())?;
                        imagens_importadas::substituir_png(&mut aba.sheet, id, &png)?;
                        if let Some(imagem) = crate::compat::imagem_de_bytes(&png) {
                            self.imagens_importadas.handles.borrow_mut().insert(id.to_owned(), imagem);
                        }
                        Ok(())
                    });
                match resultado {
                    Ok(()) => {
                        self.invalidar_cache_objetos();
                        self.alterado = true;
                        self.status = trf!("Efeito aplicado à imagem na planilha: {chave}");
                    }
                    Err(erro) => self.status = erro,
                }
                Task::none()
            }
            Msg::ImagemLaboratorioSalvar => {
                if self.efeitos_imagem.png_exportacao().is_err() {
                    self.status = tr!("Abra uma imagem antes de exportar.").into();
                    return Task::none();
                }
                Task::perform(async {
                    rfd::AsyncFileDialog::new().set_directory(dialogos::pasta_padrao())
                        .add_filter(tr!("Imagem PNG"), &["png"])
                        .set_file_name("FeatherSheet-efeito.png")
                        .save_file().await.map(|f| f.path().to_path_buf())
                }, Msg::ImagemLaboratorioDestino)
            }
            Msg::ImagemLaboratorioDestino(destino) => {
                if let Some(mut caminho) = destino {
                    caminho.set_extension("png");
                    self.status = match self.efeitos_imagem.png_exportacao()
                        .and_then(|bytes| feathersheet_io::arquivo::gravar_atomico(&caminho, &bytes)
                            .map_err(|e| trf!("Falha ao gravar PNG: {e}"))) {
                        Ok(()) => trf!("Imagem PNG exportada: {}", caminho.display()),
                        Err(e) => e,
                    };
                }
                Task::none()
            }
            Msg::ImagemLaboratorioFechar => {
                self.efeitos_imagem.aberto = false;
                Task::none()
            }
            Msg::MacroVisual(EventoVisual::SelecionarMenu(id, indice)) =>
                self.selecionar_item_menu_macro(id, indice),
            // Busca, Localizar/Substituir e desfazer do código (macros/codigo.rs).
            Msg::MacroVisual(EventoVisual::Codigo(e)) => self.evento_codigo_macro(e),
            Msg::MacroVisual(EventoVisual::Designer(macros::designer_formulario::EventoDesigner::EscolherImagem)) => self.escolher_imagem_designer(),
            Msg::MacroVisual(e) => {
                let estava = self.macro_visual.renomeando.is_some();
                self.evento_macro_visual(e);
                // Renomear (ou Nova pasta) acabou de abrir o campo: cursor nele.
                if !estava && self.macro_visual.renomeando.is_some() {
                    Task::focus("macro-renomear")
                } else { Task::none() }
            },
            Msg::MacroEditar => self.editar_codigo_macro(),
            Msg::MacroNome(nome) => {
                self.macro_nome = nome;
                if self.macro_visual.documento.starts_with("modulo:") {
                    self.macro_visual.documento = format!("modulo:{}", self.macro_nome);
                }
                Task::none()
            },
            Msg::BotaoMacroNovo => { self.novo_botao_macro(); Task::none() },
            Msg::BotaoMacroPressionar(id, gesto) => {
                self.pressionar_botao_macro(id, gesto);
                Task::none()
            },
            Msg::BotaoMacroEditar(id) => { self.editar_botao_macro(&id); Task::none() },
            Msg::BotaoMacroRotulo(rotulo) => {
                if let Some(e) = &mut self.botao_macro_modal { e.rotulo = rotulo; }
                Task::none()
            },
            Msg::BotaoMacroEscolher(nome) => {
                if let Some(e) = &mut self.botao_macro_modal { e.macro_nome = nome; }
                Task::none()
            },
            Msg::BotaoMacroSalvar => { self.guardar_botao_macro(); Task::none() },
            Msg::BotaoMacroExcluir => { self.excluir_botao_macro(); Task::none() },
            Msg::BotaoMacroCancelar => { self.botao_macro_modal = None; Task::none() },
            Msg::BotaoMacroExecutar(id) => {
                self.executar_botao_macro(id);
                Task::none()
            },
            Msg::MacroNovoModulo => {
                self.criar_modulo_macro();
                // Quando invocado pela Ribbon da planilha, abrir o editor.
                // Não alternar uma janela que já esteja aberta.
                if self.janelas.aberta(Auxiliar::Macros) {
                    Task::none()
                } else {
                    self.alternar(Auxiliar::Macros)
                }
            },
            Msg::MacroNovoFormulario => {
                self.evento_designer(super::macros::designer_formulario::EventoDesigner::Novo);
                self.update(Msg::MacroComponentes)
            }
            // Faixa > Componentes: o editor de macros aberto no desenho de formulários.
            Msg::MacroComponentes => {
                self.evento_macro_visual(EventoVisual::Documento("componente:previsao".into()));
                if self.janelas.aberta(Auxiliar::Macros) { Task::none() } else { self.alternar(Auxiliar::Macros) }
            },
            Msg::MacroGuardarModulo => { self.guardar_modulo_macro(); Task::none() },
            Msg::MacroAbrirModulo(nome) => { self.abrir_modulo_macro(&nome); Task::none() },
            Msg::MacroExcluirModulo => { self.excluir_modulo_macro(); Task::none() },
            Msg::MacroImportarCatalogo => Self::importar_catalogo_macro(),
            Msg::MacroCatalogoLido(resultado) => {
                match resultado {
                    Ok(Some(texto)) => self.mesclar_catalogo_macro(&texto),
                    Err(e) => self.macro_saida = trf!("Erro de importação: {e}"),
                    _ => {}
                }
                Task::none()
            },
            Msg::MacroExportarCatalogo => Self::exportar_catalogo_macro(),
            Msg::MacroDestinoCatalogo(path) => {
                if let Some(mut path) = path {
                    if path.extension().is_none() { path.set_extension("json"); }
                    self.macro_saida = match self.macro_catalogo.salvar_em(&path) {
                        Ok(()) => trf!("Catálogo exportado: {}", path.display()),
                        Err(e) => trf!("Erro ao exportar catálogo: {e}"),
                    };
                }
                Task::none()
            },
            Msg::MacroExecutar => { self.executar_macro(); Task::none() },
            Msg::MacroAbrir => Self::abrir_macro(),
            Msg::MacroLido(resultado) => {
                match resultado {
                    Ok(Some(codigo)) => {
                        self.macro_codigo = pelicano::Content::with_text(&codigo);
                        self.macro_visual.documento = format!("modulo:{}", self.macro_nome);
                        self.macro_saida = tr!("Macro carregada. Clique em Executar para rodar.").into();
                    },
                    Err(e) => self.macro_saida = e,
                    _ => {},
                }
                Task::none()
            },
            Msg::MacroSalvar => Self::salvar_macro(),
            Msg::MacroDestino(path) => {
                if let Some(mut path) = path {
                    if path.extension().is_none() { path.set_extension("js"); }
                    self.macro_saida = match feathersheet_io::arquivo::gravar_atomico(&path,self.macro_codigo.text().as_bytes()) {
                        Ok(()) => trf!("Macro salva em {}",path.display()),
                        Err(e) => trf!("Erro ao salvar macro: {e}"),
                    };
                }
                Task::none()
            },
            Msg::Zoom(zoom) => {
                if self.edicao_inline.is_some() && !self.confirmar() { return Task::none(); }
                self.config.zoom = self.config.zoom_limitado(zoom);
                // A origem é recalculada no Canvas para o novo tamanho lógico.
                if self.ancorar_rodape { self.linha_inicio = MAX_ROWS - 1; }
                self.config.gravar();
                Task::none()
            },
            Msg::ExplicacaoErroAbrir => {
                if !self.confirmar() { return Task::none(); }
                self.erro_explicando = Some((self.folha, self.linha, self.coluna));
                Task::none()
            }
            Msg::ExplicacaoErroFechar => {
                self.erro_explicando = None;
                Task::none()
            }
            Msg::ExplicacaoErroIrPara(indice) => {
                self.ir_para_passo_erro(indice);
                Task::none()
            }
            Msg::ValidacaoAbrir => {
                if !self.confirmar() { return Task::none(); }
                let json = self.workbook.aba(self.folha)
                    .and_then(|a| a.sheet.va.as_deref());
                let regras = match validacao_ui::carregar_regras(json) {
                    Ok(regras) => regras,
                    Err(erro) => { self.status = erro; return Task::none(); }
                };
                // Uma célula de um intervalo amplo também abre a regra já aplicada.
                // A mais recente tem prioridade, tal como o menu de opções.
                let indice = regras.iter().rposition(|v| v.ranges.iter()
                    .any(|range| range.contains(self.linha, self.coluna)));
                self.validacao_formulario = Some(
                    indice.and_then(|i| regras.get(i))
                        .map(validacao_ui::Formulario::carregar)
                        .unwrap_or_default());
                self.validacao_editando = indice;
                self.validacao_aba = validacao_ui::AbaValidacao::Configuracoes;
                self.validacao_erro = None;
                self.lista_aberta = false;
                Task::none()
            }
            Msg::ValidacaoFechar => {
                self.validacao_formulario = None;
                self.validacao_erro = None;
                self.validacao_editando = None;
                Task::none()
            }
            Msg::ListaDaColuna => {
                if self.lista_aberta { self.lista_aberta = false; self.lista_coluna = None; }
                else { self.abrir_lista_da_coluna(); }
                Task::none()
            }
            Msg::ValidacaoListaAlternar => {
                self.lista_aberta = !self.lista_aberta;
                if self.lista_aberta { self.marcar_valor_atual_na_lista(); }
                else { self.lista_coluna = None; }
                Task::none()
            }
            // A roda sobre a lista: o primeiro item à vista, já validado pela grade.
            Msg::ValidacaoListaPagina(inicio) => {
                self.lista_pagina = inicio.max(0) as usize;
                Task::none()
            }
            Msg::ValidacaoListaEscolher(valor) => {
                self.editor_formula.encerrar();
                if let Err(erro) = self.validar_entrada(self.folha, self.linha, self.coluna, &valor) {
                    self.status = erro;
                    return Task::none();
                }
                self.workbook.escrever_celula(self.folha, self.linha, self.coluna, valor.clone());
                self.edicao = valor;
                self.alterado = true;
                self.lista_aberta = false;
                self.lista_coluna = None;
                self.lista_pagina = 0;
                self.status = trf!("{} atualizado", cell_name(self.linha, self.coluna));
                Task::none()
            }
            Msg::ValidacaoPermitir(valor) => {
                if let Some(f) = &mut self.validacao_formulario { f.definir_permitir(valor); }
                self.validacao_erro = None;
                Task::none()
            }
            Msg::ValidacaoAba(aba) => { self.validacao_aba = aba; Task::none() }
            Msg::ValidacaoEv(evento) => self.evento_validacao(evento),
            Msg::Serie(evento) => self.evento_serie(evento),
            Msg::ValidacaoOperador(v) => { if let Some(f) = &mut self.validacao_formulario { f.operador = v; } Task::none() }
            Msg::ValidacaoPrimeiro(v) => { if let Some(f) = &mut self.validacao_formulario { f.primeiro = v; } Task::none() }
            Msg::ValidacaoSegundo(v) => { if let Some(f) = &mut self.validacao_formulario { f.segundo = v; } Task::none() }
            Msg::ValidacaoMensagem(v) => { if let Some(f) = &mut self.validacao_formulario { f.mensagem = v; } Task::none() }
            evento @ (Msg::ValidacaoSalvar | Msg::ValidacaoRemover) => {
                let remover = matches!(evento, Msg::ValidacaoRemover);
                let area = self.area_selecionada();
                let json_atual = self.workbook.aba(self.folha)
                    .and_then(|a| a.sheet.va.as_deref());
                let mut regras = match validacao_ui::carregar_regras(json_atual) {
                    Ok(regras) => regras,
                    Err(erro) => {
                        self.validacao_erro = Some(erro.clone());
                        self.status = erro;
                        return Task::none();
                    }
                };
                let editando = self.validacao_editando.filter(|&i| i < regras.len());
                if remover && editando.is_none() {
                    self.validacao_erro = Some(tr!("Nenhuma regra existente nesta seleção").into());
                    return Task::none();
                }
                let destino = editando.map(|i| regras[i].ranges.clone())
                    .unwrap_or_else(|| vec![area]);
                let nova = if remover { None } else {
                    match self.validacao_formulario.as_ref()
                        .expect("Formulário de validação aberto").construir(destino)
                    {
                        Ok(v) => Some(v),
                        Err(e) => {
                            self.validacao_erro = Some(e.clone());
                            self.status = e;
                            return Task::none();
                        }
                    }
                };
                match (editando, nova) {
                    (Some(i), Some(regra)) => regras[i] = regra,
                    (Some(i), None) => { regras.remove(i); }
                    (None, Some(regra)) => regras.push(regra),
                    (None, None) => return Task::none(),
                }
                let json = match serde_json::to_string(&regras) {
                    Ok(json) => json,
                    Err(e) => {
                        self.validacao_erro = Some(trf!("Não foi possível salvar: {e}"));
                        return Task::none();
                    }
                };
                self.workbook.editar(self.folha, |sheet| sheet.va = Some(json));
                self.alterado = true;
                self.atualizar_circulos();
                self.validacao_formulario = None;
                self.validacao_editando = None;
                self.validacao_erro = None;
                self.lista_aberta = false;
                self.status = if remover { tr!("Validação removida") }
                    else { tr!("Validação salva") }.into();
                Task::none()
            }
            Msg::Busca => self.abrir_busca(self.busca.substituir),
            Msg::BuscaAba(substituir) => self.abrir_busca(substituir),
            Msg::BuscaArrastoInicio => { self.busca.arrasto = Some(None); Task::none() },
            Msg::BuscaArrastoMover(p) => { self.arrastar_busca(p); Task::none() },
            Msg::BuscaArrastoFim => { self.busca.arrasto = None; Task::none() },
            Msg::BuscaMaisOpcoes => { self.busca.mais_opcoes = !self.busca.mais_opcoes; Task::none() },
            Msg::BuscaPorColunas(v) => {
                self.busca.por_colunas = v;
                self.busca.resultados.clear();
                self.busca.mostrar_todos = false;
                Task::none()
            },
            Msg::FecharBusca => { self.busca.aberta = false; self.busca.arrasto = None; Task::none() },
            Msg::BuscaTexto(t) => {
                self.busca.texto = t;
                self.busca.mostrar_todos = false;
                self.busca.resultados.clear();
                Task::none()
            },
            Msg::Substituicao(t) => { self.busca.substituicao = t; Task::none() },
            Msg::BuscaMaiusculas(v) => {
                self.busca.opcoes.match_case = v;
                self.busca.resultados.clear();
                self.busca.mostrar_todos = false;
                Task::none()
            },
            Msg::BuscaInteira(v) => {
                self.busca.opcoes.whole_cell = v;
                self.busca.resultados.clear();
                self.busca.mostrar_todos = false;
                Task::none()
            },
            Msg::BuscaFormulas(v) => {
                self.busca.opcoes.in_formulas = v;
                self.busca.resultados.clear();
                self.busca.mostrar_todos = false;
                Task::none()
            },
            Msg::BuscaPasta(v) => {
                self.busca.pasta_inteira = v;
                self.busca.resultados.clear();
                self.busca.mostrar_todos = false;
                Task::none()
            },
            Msg::Localizar => self.localizar_direcao(false),
            Msg::LocalizarAnterior => self.localizar_direcao(true),
            Msg::LocalizarTodos => { self.localizar_todos(); Task::none() },
            Msg::BuscaIrPara(indice) => {
                if let Some(destino) = self.busca.resultados.get(indice).copied() {
                    self.ir_para_resultado(destino)
                } else { Task::none() }
            },
            Msg::SubstituirAtual => self.substituir_atual(),
            Msg::SubstituirTudo => { self.substituir_tudo(); Task::none() },
            Msg::Ordenar(crescente) => { self.ordenar(crescente); Task::none() },
            Msg::CabecalhoDados(v) => { self.cabecalho_dados = v; Task::none() },
            Msg::RemoverDuplicados => self.abrir_duplicados(),
            Msg::Duplicados(ev) => self.evento_duplicados(ev),
            Msg::Impressao(ev) => self.evento_impressao(ev),
            Msg::ApararEspacos => { self.aparar_espacos(); Task::none() },
            Msg::AbrirAssistenteDivisao => self.abrir_assistente_divisao(),
            Msg::AssistenteDivisao(evento) => self.evento_assistente_divisao(evento),
            Msg::DividirTextoOutro(valor) => { self.divisao_outro = valor; Task::none() },
            Msg::AlternarMenuOrientacao => {
                self.orientacao_menu_aberto = !self.orientacao_menu_aberto;
                if self.orientacao_menu_aberto {
                    self.bordas_menu_aberto = false;
                    self.colar_menu_aberto = false;
                    self.condicional_menu_aberto = false;
                    self.menu_aberto = false;
                    self.cor_ativa = None;
                    self.contexto = None;
                    self.orientacao_popup_x = self.ponteiro_x - 34.;
                    self.orientacao_popup_y = self.ponteiro_y + 24.;
                }
                Task::none()
            }
            Msg::FecharMenuOrientacao => {
                self.orientacao_menu_aberto = false;
                Task::none()
            }
            Msg::OrientacaoEscolher(opcao) => self.escolher_orientacao(opcao),
            Msg::FormatarCelulasAbrir => self.abrir_formatar_celulas(formatar_celulas::Aba::Numero),
            Msg::FormatoCelula(evento) => self.evento_formatar_celulas(evento),
            Msg::AlternarMenuBordas => {
                self.bordas_menu_aberto = !self.bordas_menu_aberto;
                if self.bordas_menu_aberto {
                    self.menu_aberto = false;
                    self.colar_menu_aberto = false;
                    self.orientacao_menu_aberto = false;
                    self.divisao_personalizada = false;
                    self.condicional_menu_aberto = false;
                    self.cor_ativa = None;
                    self.contexto = None;
                    self.tabela_popup_filtro = false;
                    // Posição logo abaixo da seta da Ribbon, sem deslocar a faixa.
                    self.bordas_popup_x = self.ponteiro_x - 20.0;
                    self.bordas_popup_y = self.ponteiro_y + 24.0;
                }
                Task::none()
            },
            Msg::FecharMenuBordas => {
                self.bordas_menu_aberto = false;
                Task::none()
            },
            // Compatibilidade com atalhos antigos: nunca mostrar o menu suspenso.
            Msg::AlternarMenuDivisao => self.abrir_assistente_divisao(),
            Msg::DividirTextoAplicar => {
                let separador = self.divisao_outro.clone();
                self.dividir_texto(&separador);
                if !separador.is_empty() {
                    self.divisao_personalizada = false;
                }
                Task::none()
            },
            Msg::AlternarMenuEdicao(tipo) => {
                self.edicao_menu = if self.edicao_menu == Some(tipo) {
                    None
                } else {
                    Some(tipo)
                };
                if self.edicao_menu.is_some() {
                    self.edicao_popup_x = self.ponteiro_x - 30.0;
                    // Botões pequenos (AutoSoma, Preencher, Limpar) ficam nas
                    // linhas de cima da faixa: o menu cola na borda do botão.
                    // Os botões grandes e as setas de baixo ficam na base da faixa.
                    self.edicao_popup_y = if matches!(tipo, "autosoma" | "preencher" | "limpar" | "validacao" | "sublinhado") {
                        self.ponteiro_y + 14.0
                    } else {
                        43.0 + 52.0 + faixa_nativa::ALTURA_FAIXA_NATIVA - 4.0
                    };
                    self.bordas_menu_aberto = false;
                    self.colar_menu_aberto = false;
                    self.tabelas_menu_aberto = false;
                    self.estilos_menu_aberto = false;
                    self.condicional_menu_aberto = false;
                    self.menu_aberto = false;
                }
                Task::none()
            }
            Msg::FecharMenuEdicao => { self.edicao_menu = None; Task::none() },
            Msg::InserirSimbolo(s) => self.inserir_simbolo(&s),
            Msg::EdicaoMenuAcao(msg) => {
                self.edicao_menu = None;
                self.update(*msg)
            }
            Msg::AutoSoma => { self.edicao_menu = None; self.autosoma("SOMA"); Task::none() },
            Msg::AutoConta(funcao) => { self.edicao_menu = None; self.autosoma(funcao); Task::none() },
            Msg::Preencher(direcao) => {
                self.preencher_selecao(direcao);
                Task::none()
            },
            Msg::PreenchimentoRelampago => { self.preencher_relampago(); Task::none() }
            Msg::CaixasInserir => { self.alternar_botao_caixas(); Task::none() }
            Msg::CaixaAlternar(r, c) => { self.alternar_caixa(r, c); Task::none() }
            Msg::PreencherAlca(destino) => {
                self.preencher_pela_alca(destino);
                Task::none()
            },
            Msg::Novo => { self.menu_aberto = false; self.proteger(Pendente::Novo) },
            Msg::NovoDeModelo(id) => { self.menu_aberto = false; self.proteger(Pendente::Modelo(id)) },
            Msg::Abrir => {
                self.menu_aberto = false;
                if self.carregando {
                    self.status = tr!("Uma planilha já está sendo importada. Aguarde concluir ou cancelar.").into();
                    Task::none()
                } else {
                    self.status = tr!("Selecione uma planilha para abrir…").into();
                    self.proteger(Pendente::Abrir)
                }
            },
            Msg::BackupCriar => self.criar_backup(),
            Msg::BackupCriado(resultado) => {
                self.status = match resultado {
                    Ok(path) => trf!("Cópia de segurança criada: {}", path.display()),
                    Err(e) => trf!("Não foi possível criar o backup: {e}"),
                };
                Task::none()
            },
            Msg::RecuperarTemporario => {
                self.menu_aberto = false;
                if !Self::existem_temporarios() {
                    self.status = tr!("Nenhuma cópia temporária encontrada em Documentos/FeatherSheet/Temporarios.").into();
                    Task::none()
                } else if self.carregando {
                    self.status = tr!("Aguarde a planilha em carregamento.").into();
                    Task::none()
                } else {
                    self.proteger(Pendente::RecuperarTemporario)
                }
            }
            Msg::DestinoTemporario(Some(path)) => {
                self.carregando = true;
                self.status = tr!("Recuperando cópia temporária…").into();
                Self::tarefa_abrir_recente(path, self.config.separador_csv())
            }
            Msg::DestinoTemporario(None) => {
                self.status = tr!("Recuperação cancelada; planilha preservada.").into();
                Task::none()
            }
            Msg::ArquivoEscolhido(None) => self.update(Msg::Aberto(Ok(None))),
            Msg::ArquivoEscolhido(Some(path)) => {
                // Sai da tela inicial já: o rodapé mostra o progresso da leitura.
                self.em_tela_inicio = false;
                self.carregando = true;
                self.status = trf!("Abrindo {}", path.display());
                Self::tarefa_abrir_recente(path, self.config.separador_csv())
            }
            Msg::Aberto(resultado) => {
                self.carregando = false;
                self.recolher_diario_motor();
                match resultado {
                    Ok(Some(pasta)) => {
                        let dados = pasta.dados.lock().ok().and_then(|mut guard| guard.take());
                        if let Some((workbook, folha)) = dados {
                            self.instalar(pasta.caminho, workbook, folha);
                            // Tempo real de leitura (sem o diálogo) e a memória ao final, no console.
                            let memoria = Self::rss_atual_kb()
                                .map(|kb| trf!(" · {:.0} MB de RAM", kb as f64 / 1024.0))
                                .unwrap_or_default();
                            self.status = trf!("{} · carregado em {:.1} s{memoria}",
                                self.status, pasta.ms as f64 / 1000.0);
                        } else {
                            self.status = tr!("Não foi possível receber a planilha carregada").into();
                        }
                    }
                    Ok(None) => self.status = tr!("Abertura cancelada; planilha preservada").into(),
                    Err(e) => self.status = e,
                }
                Task::none()
            }
            Msg::Salvar => {
                self.menu_aberto = false;
                self.confirmar();
                if let Some(path) = self.caminho.clone() { self.gravar(path) }
                else { self.dialogo_salvar() }
            }
            Msg::SalvarComo => { self.menu_aberto = false; self.confirmar(); self.dialogo_salvar() },
            Msg::MenuSalvarCopia => {
                self.menu_aberto = false;
                self.dialogo_salvar_copia()
            }
            Msg::DestinoCopia(Some(path)) => match documento_operacoes::com_extensao_fshz(path) {
                Ok(path) => self.salvar_copia(path),
                Err(e) => { self.status = e; Task::none() }
            },
            Msg::DestinoCopia(None) => Task::none(),
            Msg::Gravado(path, copia, resultado) => self.gravacao_terminou(path, copia, resultado),
            Msg::Recuperacao(evento) => self.evento_recuperacao(evento),
            Msg::RecuperacaoAutomatica(ativo) => {
                self.config.recuperacao_automatica = ativo;
                self.config.gravar();
                if !ativo { self.descartar_recuperacao(); }
                Task::none()
            }
            Msg::ExportarXlsx => { self.menu_aberto = false; self.confirmar(); self.dialogo_exportar_xlsx() },
            Msg::DestinoXlsx(Some(mut path)) => {
                if path.extension().and_then(|s|s.to_str()) != Some("xlsx") { path.set_extension("xlsx"); }
                self.gravar_xlsx(path); Task::none()
            }
            Msg::DestinoXlsx(None) => Task::none(),
            Msg::ExportarJson => {
                self.menu_aberto = false;
                self.confirmar();
                self.dialogo_exportar_json()
            }
            Msg::DestinoJson(Some(mut path)) => {
                if !path.extension().and_then(|s| s.to_str()).is_some_and(|s| s.eq_ignore_ascii_case("json")) {
                    path.set_extension("json");
                }
                self.gravar_json(path);
                Task::none()
            }
            Msg::DestinoJson(None) => Task::none(),
            Msg::ExportarDelimitado(tsv) => {
                self.menu_aberto=false;
                self.confirmar();
                self.dialogo_exportar_delimitado(tsv)
            }
            Msg::DestinoDelimitado(Some(mut path),tsv) => {
                let ext=if tsv{"tsv"}else{"csv"};
                if path.extension().and_then(|s|s.to_str())!=Some(ext) {
                    path.set_extension(ext);
                }
                self.gravar_delimitado(path,tsv);
                Task::none()
            }
            Msg::DestinoDelimitado(None,_) => Task::none(),
            Msg::DestinoSalvar(Some(path)) => match documento_operacoes::com_extensao_fshz(path) {
                Ok(path) => self.gravar(path),
                Err(e) => { self.pendente = None; self.status = e; Task::none() }
            },
            Msg::DestinoSalvar(None) => { self.pendente = None; Task::none() },
            Msg::TelaInteira => {
                self.tela_inteira = !self.tela_inteira;
                match self.janelas.principal {
                    Some(id) => Task::window_of(id, pelicano::WindowCmd::Fullscreen(self.tela_inteira)),
                    None => Task::none(),
                }
            }
            Msg::RenomearArquivo => { self.menu_aberto = false; self.confirmar(); self.dialogo_renomear() },
            Msg::DestinoRenomear(Some(path)) => { self.renomear_arquivo_atual(path); Task::none() },
            Msg::DestinoRenomear(None) => Task::none(),
            Msg::EditarNaCelula(r, c, x, y, largura, altura) => {
                // A grade informa a geometria do Canvas, sem duplicar cálculos
                // de rolagem/congelamento. Usamos a mesma validação e histórico
                // da barra fx: muda apenas onde o usuário digita.
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                if !self.confirmar() { return Task::none(); }
                // Num valor de tabela dinâmica o duplo clique mostra os
                // detalhes (as linhas da origem), como no Excel.
                if self.detalhar_dinamica(r, c) { return Task::none(); }
                self.trocar_selecao_por_clique(r, c);
                self.editor_formula.ativo = true;
                self.editor_formula.origem = Some((self.folha, self.linha, self.coluna));
                self.editor_formula.modo_texto = true;
                self.editor_formula.modo_apontar = false;
                self.editor_formula.posicionar(&self.edicao, self.edicao.chars().count());
                self.edicao_inline = Some(EdicaoInline {
                    folha: self.folha, linha: self.linha, coluna: self.coluna,
                    x, y, largura, altura,
                });
                if self.edicao.contains('\n') {
                    let conteudo = pelicano::Content::with_text(&self.edicao);
                    // shortcut: o Content do pelicano só leva o cursor ao começo de uma linha; aqui
                    // fica no começo da última, não no fim do texto. Melhorar quando houver mover ao fim.
                    conteudo.go_to_line(usize::MAX);
                    self.multilinha = Some(conteudo);
                    crate::campo_fx::focar_multilinha()
                } else {
                    self.multilinha = None;
                    // O campo recém-focado já põe o cursor no fim do valor.
                    Task::focus("feathersheet.celula")
                }
            }
            Msg::Selecionar(r,c) => {
                // Atingir meta aberto: o clique também preenche o campo ativo.
                self.atingir_meta_clique(r, c);
                self.tabela_dados_clique(r, c);
                self.cenarios_clique(r, c);
                // Clicar na grade encerra a edição inline da guia, como no Excel.
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                self.imagem_selecionada = None;
                self.arraste_imagem = None;
                self.botao_macro_selecionado = None;
                self.arraste_botao_macro = None;
                let outra=self.editor_formula.origem.and_then(|(origem,_,_)|
                    (origem!=self.folha).then(||self.workbook.nomes().get(self.folha).cloned()).flatten());
                // Mesmo caminho de referência para a própria aba; prefixo apenas
                // quando o apontamento acontece em outra aba.
                let inseriu_referencia = self.editor_formula.pode_apontar(&self.edicao) &&
                    match outra.as_deref() {
                        Some(nome) => self.editor_formula.referencia_na_aba(
                            &mut self.edicao, r, c, false, Some(nome)),
                        None => self.editor_formula.referencia(&mut self.edicao, r, c, false),
                    };
                if inseriu_referencia {
                    // shortcut: o text_input do pelicano não recebe a posição do cursor por Task: ele fica onde
                    // estava (ou no fim). Melhorar quando o pelicano tiver mover cursor por id.
                    return Task::focus("feathersheet.fx");
                }
                if !self.confirmar() { return Task::none(); }
                self.trocar_selecao_por_clique(r,c); Task::none()
            }
            Msg::SelecionarLinha(r) => {
                // Fórmula aberta: o cabeçalho escreve a linha inteira (5:5).
                let r = r.min(MAX_ROWS-1);
                if let Some(tarefa) = self.apontar_faixa((r, 0), (r, MAX_COLS-1)) { return tarefa; }
                self.selecoes_extras.clear();
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                self.confirmar();
                self.linha = r.min(MAX_ROWS-1);
                self.coluna = 0;
                self.fim_selecao = (self.linha, MAX_COLS-1);
                self.endereco = endereco_area(&self.area_selecionada());
                Task::none()
            }
            Msg::SelecionarColuna(c) => {
                // Fórmula aberta: o cabeçalho escreve a coluna inteira (C:C).
                let c = c.min(MAX_COLS-1);
                if let Some(tarefa) = self.apontar_faixa((0, c), (MAX_ROWS-1, c)) { return tarefa; }
                self.selecoes_extras.clear();
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                self.confirmar();
                self.coluna = c.min(MAX_COLS-1);
                self.linha = 0;
                self.fim_selecao = (MAX_ROWS-1, self.coluna);
                self.endereco = endereco_area(&self.area_selecionada());
                Task::none()
            }
            Msg::EstenderColuna(c) => {
                if let Some((_, c0)) = self.editor_formula.ancora {
                    if let Some(tarefa) = self.apontar_faixa((0, c0), (MAX_ROWS-1, c.min(MAX_COLS-1))) { return tarefa; }
                }
                self.confirmar();
                if !self.selecao_e_colunas_inteiras() {
                    // Âncora na coluna ativa quando antes não havia colunas inteiras.
                    self.linha = 0;
                }
                self.fim_selecao = (MAX_ROWS-1, c.min(MAX_COLS-1));
                let area = self.area_selecionada();
                self.endereco = endereco_area(&area);
                Task::none()
            }
            Msg::EstenderLinha(r) => {
                if let Some((r0, _)) = self.editor_formula.ancora {
                    if let Some(tarefa) = self.apontar_faixa((r0, 0), (r.min(MAX_ROWS-1), MAX_COLS-1)) { return tarefa; }
                }
                self.confirmar();
                if !self.selecao_e_linhas_inteiras() { self.coluna = 0; }
                self.fim_selecao = (r.min(MAX_ROWS-1), MAX_COLS-1);
                let area = self.area_selecionada();
                self.endereco = endereco_area(&area);
                Task::none()
            }
            Msg::SelecionarRegiaoAtual => { self.selecionar_regiao_atual(); Task::none() },
            Msg::SelecionarTudo => {
                self.selecoes_extras.clear();
                self.confirmar();
                self.linha = 0;
                self.coluna = 0;
                self.fim_selecao = (MAX_ROWS-1, MAX_COLS-1);
                self.endereco = endereco_area(&self.area_selecionada());
                Task::none()
            }
            Msg::ArrastarReferencia(inicio, pegada, de, para) => {
                // Só o rascunho é alterado: Enter/✓ ainda confirma a célula.
                if !self.editor_formula.ativo || !self.edicao.starts_with('=') {
                    return Task::none();
                }
                let caixas = feathersheet_core::formula::ref_drag::reference_boxes(&self.edicao);
                let Some(caixa) = caixas.into_iter().find(|c| c.start == inicio) else {
                    return Task::none();
                };
                let aba_atual = self.workbook.aba(self.folha).map(|a| a.sheet.name.as_str());
                let na_origem = self.editor_formula.origem
                    .is_some_and(|(origem,_,_)| origem == self.folha);
                let visivel = match caixa.sheet.as_deref() {
                    Some(nome) => aba_atual.is_some_and(|a| a.eq_ignore_ascii_case(nome)),
                    None => na_origem,
                };
                if !visivel { return Task::none(); }
                let rect = feathersheet_core::formula::ref_drag::drag_rect(
                    &caixa.rect, pegada, de, para);
                let novo = feathersheet_core::formula::ref_drag::rewrite_box(
                    &self.edicao, &caixa, &rect);
                if novo != self.edicao {
                    let final_ref = caixa.end + novo.chars().count() - self.edicao.chars().count();
                    self.edicao = novo;
                    let pos = final_ref.min(self.edicao.chars().count());
                    self.editor_formula.posicionar(&self.edicao, pos);
                    self.editor_formula.modo_texto = true;
                    self.editor_formula.modo_apontar = false;
                    // shortcut: o text_input do pelicano não recebe a posição do cursor por Task: ele fica onde
                    // estava (ou no fim). Melhorar quando o pelicano tiver mover cursor por id.
                    return Task::focus("feathersheet.fx");
                }
                Task::none()
            }
            Msg::Estender(r,c) => {
                let outra=self.editor_formula.origem.and_then(|(origem,_,_)|
                    (origem!=self.folha).then(||self.workbook.nomes().get(self.folha).cloned()).flatten());
                if self.editor_formula.ativo && self.editor_formula.ancora.is_some() &&
                    self.editor_formula.referencia_na_aba(&mut self.edicao,r,c,true,outra.as_deref()) {
                    // shortcut: o text_input do pelicano não recebe a posição do cursor por Task: ele fica onde
                    // estava (ou no fim). Melhorar quando o pelicano tiver mover cursor por id.
                    return Task::focus("feathersheet.fx");
                }
                self.confirmar();
                self.fim_selecao = (r.min(MAX_ROWS-1), c.min(MAX_COLS-1));
                let area = self.area_selecionada();
                self.endereco = endereco_area(&area);
                Task::none()
            }
            Msg::PosicionarCursorFx(pos) => {
                if !self.editor_formula.ativo {
                    self.editor_formula.ativo=true;
                    self.editor_formula.origem=Some((self.folha,self.linha,self.coluna));
                }
                self.editor_formula.posicionar(&self.edicao,pos);
                self.editor_formula.modo_texto=true;
                self.editor_formula.modo_apontar=false;
                Task::none()
            }
            Msg::SelecionarTextoFx(inicio,fim) => {
                if !self.editor_formula.ativo {
                    self.editor_formula.ativo=true;
                    self.editor_formula.origem=Some((self.folha,self.linha,self.coluna));
                }
                self.editor_formula.selecionar_texto(&self.edicao,inicio.min(fim),inicio.max(fim));
                self.editor_formula.modo_texto=true;
                self.editor_formula.modo_apontar=false;
                Task::none()
            }
            Msg::InserirQuebraLinha => {
                if !self.editor_formula.ativo { return Task::none(); }
                let cursor = self.editor_formula.posicao(&self.edicao);
                let (inicio, fim) = self.editor_formula.selecao_texto
                    .unwrap_or((cursor, cursor));
                let a = self.edicao.char_indices().nth(inicio)
                    .map(|(i, _)| i).unwrap_or(self.edicao.len());
                let b = self.edicao.char_indices().nth(fim)
                    .map(|(i, _)| i).unwrap_or(self.edicao.len());
                self.edicao.replace_range(a..b, "\n");
                self.editor_formula.digitou();
                let pos = inicio + 1;
                self.editor_formula.cursor = Some(pos);
                self.editor_formula.modo_texto = true;
                let conteudo = pelicano::Content::with_text(&self.edicao);
                // O cursor fica logo depois da quebra: o começo da linha seguinte.
                conteudo.go_to_line(self.edicao.chars().take(pos).filter(|&c| c == '\n').count() + 1);
                self.multilinha = Some(conteudo);
                crate::campo_fx::focar_multilinha()
            }
            Msg::EditarMultilinha => {
                if let Some(conteudo) = self.multilinha.as_ref() {
                    // O próprio widget já editou o conteúdo; o texto sai com as quebras exatas.
                    let novo = conteudo.text();
                    self.editor_formula.alterado(&self.edicao, &novo);
                    self.edicao = novo;
                }
                Task::none()
            }
            Msg::Editar(t) => {
                self.multilinha = None;
                if !self.editor_formula.ativo {
                    self.editor_formula.origem=Some((self.folha,self.linha,self.coluna));
                }
                self.editor_formula.ativo=true;
                self.editor_formula.alterado(&self.edicao,&t);
                self.edicao=t;
                Task::none()
            }
            Msg::Digitar(t) => {
                self.multilinha = None;
                if !self.editor_formula.ativo {
                    self.editor_formula.origem=Some((self.folha,self.linha,self.coluna));
                }
                self.editor_formula.ativo=true;
                self.editor_formula.alterado(&self.edicao,&t);
                self.edicao=t;
                // O campo recém-focado já põe o cursor no fim do valor.
                Task::focus("feathersheet.fx")
            }
            Msg::AceitarSugestao(i) => {
                if self.editor_formula.aceitar_sugestao(&mut self.edicao,i,self.config.idioma=="en-US") {
                    // O caret vai para depois do "(" (=HO + Tab → =HOJE(|).
                    let pos = self.editor_formula.cursor.unwrap_or(self.edicao.chars().count());
                    crate::campo_fx::posicionar(pos)
                } else { Task::none() }
            }
            Msg::NavegarSugestao(delta) => {
                let n=self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US").len();
                if n>0 {
                    self.editor_formula.sugestao=(self.editor_formula.sugestao as i32+delta)
                        .rem_euclid(n as i32) as usize;
                }
                Task::none()
            }
            Msg::EnviarEditor => {
                if !self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US").is_empty() {
                    // Enquanto houver autocomplete, Enter insere a sugestão.
                    self.update(Msg::AceitarSugestao(self.editor_formula.sugestao))
                } else {
                    // Enter conclui a edição (texto ou fórmula), move uma
                    // linha e devolve teclado à grade, como numa planilha.
                    let estava_editando=self.editor_formula.ativo;
                    // Ctrl+Enter: o texto vai para a seleção inteira e a
                    // célula ativa não sai do lugar.
                    if self.ctrl_pressionado && self.preencher_selecao_com_edicao() {
                        return crate::campo_fx::desfocar();
                    }
                    if !self.confirmar() { return Task::none(); }
                    if estava_editando {
                        let navegacao=self.update(Msg::Mover(1,0));
                        Task::batch([navegacao,crate::campo_fx::desfocar()])
                    } else {
                        crate::campo_fx::desfocar()
                    }
                }
            }
            // ✓ confirma no endereço atual, mas também libera o foco de fx.
            Msg::Confirmar => {
                if self.confirmar() { crate::campo_fx::desfocar() }
                else { Task::none() }
            }
            Msg::Rolar(dr,dc) => {
                // Geometria do overlay é relativa ao viewport atual.
                if self.edicao_inline.is_some() && !self.confirmar() { return Task::none(); }
                self.ancorar_rodape = false;
                // Rolagens de comandos externos mantêm limites de células.
                // O Canvas faz a correção fina para a altura/zoom reais.
                self.contexto = None;
                let (fixas_linhas, _) = self.congelamento_atual();
                // Com a grade já desenhada, o mesmo passo da roda sobre ela: salta linhas
                // ocultas e para no fim visível. Somando cru, a roda sobre uma imagem ou
                // gráfico movia o objeto com a grade parada (relato de 26/10/2026).
                let altura = self.eixos_grade.borrow().tamanho_canvas.map(|t| t.height).filter(|h| *h > 0.0);
                self.linha_inicio = match altura {
                    Some(h) => self.grade().origem_apos_rolar(h, dr),
                    None => (self.linha_inicio as i64 + dr as i64)
                        .clamp(fixas_linhas as i64, MAX_ROWS as i64-1) as u32,
                };
                // Não atravessa colunas ocultas em passos de largura zero.
                self.rolar_colunas_visiveis(dc);
                Task::none()
            }
            Msg::RolarPara(linha, coluna) => {
                if self.edicao_inline.is_some() && !self.confirmar() { return Task::none(); }
                self.ancorar_rodape = false;
                self.contexto = None;
                let (fixas_linhas, fixas_colunas) = self.congelamento_atual();
                if let Some(l) = linha { self.linha_inicio = l.max(fixas_linhas).min(MAX_ROWS - 1); }
                if let Some(c) = coluna { self.coluna_inicio = c.max(fixas_colunas).min(MAX_COLS - 1); }
                Task::none()
            }
            Msg::RolarNaGrade(primeira_linha,primeira_coluna,dc,esta_no_rodape) => {
                if self.edicao_inline.is_some() && !self.confirmar() { return Task::none(); }
                self.ancorar_rodape = esta_no_rodape;
                // A roda utiliza a origem efetiva calculada no Canvas.
                // Assim, depois de Ir para A10000000, rolar para cima responde
                // imediatamente, sem avançar para além de 10 milhões.
                self.contexto = None;
                let (fixas_linhas, _) = self.congelamento_atual();
                self.linha_inicio = primeira_linha.max(fixas_linhas).min(MAX_ROWS - 1);
                // Parte da coluna de fato visível: depois de Ctrl+→ até XFD, rolar
                // para a esquerda responde já no primeiro giro.
                if dc != 0 { self.coluna_inicio = primeira_coluna; }
                self.rolar_colunas_visiveis(dc);
                Task::none()
            }
            Msg::ControleZoom(pressionado) => {
                self.ctrl_pressionado = pressionado;
                Task::none()
            }
            Msg::Roda(delta) => {
                if self.ctrl_pressionado && self.edicao_inline.is_some() && !self.confirmar() {
                    return Task::none();
                }
                if self.ctrl_pressionado {
                    if let Some(proximo) = zoom::proximo(self.config.zoom, delta,
                        self.config.zoom_minimo, self.config.zoom_maximo) {
                        self.config.zoom = proximo;
                        if self.ancorar_rodape { self.linha_inicio = MAX_ROWS - 1; }
                        self.config.gravar();
                    }
                }
                Task::none()
            }
            Msg::TeclaLiberada(key) => {
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::Control)) {
                    self.ctrl_pressionado = false;
                }
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::Shift)) {
                    self.imagens_importadas.shift.set(false);
                }
                if matches!(key, keyboard::Key::Named(keyboard::key::Named::Alt)) {
                    self.imagens_importadas.alt.set(false);
                }
                Task::none()
            }
            Msg::Copiar => match self.copiar_selecao() {
                Some(dados) => {
                    self.marca_copia = Some((self.folha, self.area_selecionada().normalized()));
                    Task::write_clipboard(dados)
                }
                None => Task::none(),
            },
            Msg::Recortar => {
                match self.copiar_selecao() {
                    Some(dados) => {
                        if let Some(copia) = self.area_copiada.as_mut() { copia.recortada = true; }
                        // A origem é limpa agora: não fica marca tracejada.
                        self.marca_copia = None;
                        // Recortar remove também o estilo direto da origem.
                        let area = self.area_selecionada();
                        self.workbook.editar(self.folha, |sheet| sheet.clear(&area));
                        self.alterado = true;
                        self.trocar_selecao(self.linha, self.coluna);
                        Task::write_clipboard(dados)
                    }
                    None => Task::none(),
                }
            }
            Msg::AlternarMenuColagem => {
                self.colar_menu_aberto = !self.colar_menu_aberto;
                if self.colar_menu_aberto {
                    self.divisao_personalizada = false;
                    self.bordas_menu_aberto = false;
                    self.menu_aberto = false;
                    self.contexto = None;
                    self.cor_ativa = None;
                    self.tabela_popup_filtro = false;
                }
                Task::none()
            }
            Msg::FecharMenuColagem => {
                self.colar_menu_aberto = false;
                Task::none()
            }
            Msg::ColarSolicitar => {
                self.colar_menu_aberto = false;
                Task::read_clipboard(|v| Msg::ColarTextoLido(v.unwrap_or_default()))
            }
            Msg::ColarTextoLido(texto) => {
                // Texto tem prioridade (copiar células pode deixar também um bitmap
                // no Windows). Sem texto e sem cópia interna, tenta imagem (print).
                if texto.is_empty() && self.area_copiada.is_none() {
                    Task::perform(async {
                        tokio::task::spawn_blocking(imagens_importadas::ler_png_da_area_de_transferencia)
                            .await.ok().flatten()
                    }, Msg::ColarImagemLida)
                } else {
                    self.update(Msg::Colar(texto))
                }
            }
            Msg::ColarImagemLida(png) => match png {
                Some(bytes) => self.update(Msg::ImagemLaboratorioCarregada(
                    Ok(Some((bytes, tr!("Captura de tela.png").to_string()))))),
                None => self.update(Msg::Colar(String::new())),
            },
            Msg::Pincel => { self.capturar_pincel(); Task::none() },
            Msg::AplicarPincel => {
                // Com a caneta de borda ligada, o fim do arraste desenha a borda.
                if !self.aplicar_desenho_borda() { self.aplicar_pincel(); }
                Task::none()
            },
            Msg::BordaDesenho(evento) => self.evento_desenho_borda(evento),
            Msg::LimparHiperlinks => self.limpar_hiperlinks_selecao(),
            Msg::ColarValoresSolicitar =>
                Task::read_clipboard(|v| Msg::ColarValores(v.unwrap_or_default())),
            Msg::ColarFormatoSolicitar =>
                Task::read_clipboard(|v| Msg::ColarFormato(v.unwrap_or_default())),
            Msg::ColarEspecialSolicitar(modo) => {
                self.colar_menu_aberto = false;
                Task::read_clipboard(move |v| Msg::ColarEspecial(v.unwrap_or_default(), modo))
            }
            Msg::ColarEspecial(dados, modo) => {
                let texto = if dados.is_empty() {
                    self.area_copiada.as_ref().map(|a| a.texto.clone()).unwrap_or_default()
                } else { dados };
                self.colar_modo(texto, modo);
                Task::none()
            }
            Msg::Colar(dados) => {
                let texto = if dados.is_empty() {
                    self.area_copiada.as_ref().map(|a| a.texto.clone()).unwrap_or_default()
                } else { dados };
                self.colar(texto);
                Task::none()
            }
            Msg::ColarValores(dados) => {
                let texto = if dados.is_empty() {
                    self.area_copiada.as_ref().map(|a| a.texto.clone()).unwrap_or_default()
                } else { dados };
                self.colar_modo(texto, ModoColagem::SomenteValores);
                Task::none()
            }
            Msg::ColarFormato(dados) => {
                let texto = if dados.is_empty() {
                    self.area_copiada.as_ref().map(|a| a.texto.clone()).unwrap_or_default()
                } else { dados };
                self.colar_modo(texto, ModoColagem::SomenteFormato);
                Task::none()
            }
            Msg::Mesclar => {
                self.confirmar();
                let area = self.area_selecionada();
                let ja_mesclada = self.workbook.aba(self.folha).is_some_and(|aba|
                    mesclas::exatamente_mesclada(&aba.sheet, &area));
                if ja_mesclada {
                    let mut mudou = false;
                    self.workbook.editar(self.folha, |sheet| {
                        mudou = mesclas::desmesclar(sheet, area.clone());
                    });
                    self.alterado |= mudou;
                    self.status = if mudou { tr!("Mesclagem desfeita").into() }
                        else { tr!("Não foi possível desmesclar a seleção").into() };
                } else if area.area() < 2 {
                    self.status = tr!("Selecione duas ou mais células para mesclar").into();
                } else {
                    let mut mudou = false;
                    self.workbook.editar(self.folha, |sheet| {
                        mudou = mesclas::mesclar(sheet, area.clone());
                    });
                    self.alterado |= mudou;
                    self.status = if mudou {
                        trf!("Células {}:{} mescladas",
                            cell_name(area.r0, area.c0), cell_name(area.r1, area.c1))
                    } else { tr!("Não foi possível mesclar a seleção").into() };
                }
                Task::none()
            }
            Msg::InclinarTexto => {
                let rotacao = self.workbook.aba(self.folha)
                    .and_then(|aba| aba.sheet.estilo_resolvido(self.linha, self.coluna))
                    .and_then(|estilo| estilo.rotation).unwrap_or_default();
                // Clique: normal -> 45 graus -> normal, na seleção inteira.
                let nova = if rotacao == 45 {
                    Campo::Nulo
                } else {
                    Campo::Numero(45.0)
                };
                self.aplicar_estilo_menu("rotation", nova);
                self.status = if rotacao == 45 {
                    tr!("Orientação normal do texto").into()
                } else {
                    tr!("Texto inclinado em 45°").into()
                };
                Task::none()
            }
            Msg::Limpar => { self.limpar_celula(); Task::none() }
            Msg::Desfazer => { self.historico(false); Task::none() }
            Msg::Refazer => { self.historico(true); Task::none() }
            restante => self.processar_mensagem_secundaria(restante),
        }
    }
}
