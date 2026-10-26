//! Tratamento de atalhos e teclado da janela principal.
use super::*;

impl App {
    pub(super) fn processar_teclado(&mut self, id: window::Id, key: keyboard::Key, mods: keyboard::Modifiers) -> Task<Msg> {
                // Enter/Esc depois de apontar uma célula para a fórmula de outra
                // janela: confirmam ou cancelam lá (como no Excel).
                if self.janelas.principal == Some(id) {
                    if let keyboard::Key::Named(tecla @ (keyboard::key::Named::Enter | keyboard::key::Named::Escape)) = &key {
                        let enter = *tecla == keyboard::key::Named::Enter;
                        if self.tecla_para_quem_aponta(enter) {
                            self.status = if enter { tr!("Fórmula da outra janela confirmada.") }
                                else { tr!("Fórmula da outra janela cancelada.") }.into();
                            return Task::none();
                        }
                    }
                }
                self.imagens_importadas.shift.set(mods.shift());
                self.imagens_importadas.alt.set(mods.alt());
                if self.pendente.is_some() && self.janelas.principal == Some(id) {
                    return if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        self.update(Msg::Cancelar)
                    } else { Task::none() };
                }
                // Alerta de validação e Série…: Enter e Esc são deles; o resto não vaza para a grade.
                if self.janelas.principal == Some(id) {
                    if let Some(tarefa) = self.tecla_validacao_serie(&key) { return tarefa; }
                }
                if self.janelas.principal==Some(id) && key==keyboard::Key::Named(keyboard::key::Named::Escape)
                    && !self.editor_formula.ativo {
                    // Aba Desenhar: fecha o menu da caneta, depois sai do modo desenho.
                    if let Some(tarefa) = self.esc_desenhar() { return tarefa; }
                    // Formas: cancela a inserção, fecha menu/painel ou tira a seleção.
                    if self.esc_objetos() { return Task::none(); }
                }
                // Delete com traços escolhidos pelo laço apaga a tinta, não as células.
                if self.janelas.principal==Some(id) && !self.editor_formula.ativo
                    && matches!(key, keyboard::Key::Named(keyboard::key::Named::Delete | keyboard::key::Named::Backspace))
                    && self.apagar_selecao_tinta() {
                    return Task::none();
                }
                // Forma selecionada: Delete exclui, setas movem 1 px, F2 edita o
                // texto, Ctrl+C/X/V/D copiam, recortam, colam e duplicam a forma.
                if self.janelas.principal==Some(id) && !self.editor_formula.ativo
                    && self.teclado_objetos(&key, mods) {
                    return Task::none();
                }
                if self.janelas.principal==Some(id) && key==keyboard::Key::Named(keyboard::key::Named::Escape) {
                    if self.tabelas_menu_aberto || self.estilos_menu_aberto {
                        return self.update(Msg::FecharMenusEstilos);
                    }
                    if self.orientacao_menu_aberto { return self.update(Msg::FecharMenuOrientacao); }
                    if self.faixa_grupo_aberto.is_some() { return self.update(Msg::FecharGrupoFaixa); }
                    if self.faixa_extras_aberto { return self.update(Msg::FecharMenuExtras); }
                    if self.bordas_menu_aberto { return self.update(Msg::FecharMenuBordas); }
                    if self.borda_desenho.modo.is_some() {
                        return self.update(Msg::BordaDesenho(desenho_borda_ui::Evento::Sair));
                    }
                    if self.colar_menu_aberto { return self.update(Msg::FecharMenuColagem); }
                    if self.estatisticas.is_some() { return self.update(Msg::EstatisticasFechar); }
                    if self.perfil_coluna.is_some() { return self.update(Msg::PerfilColunaFechar); }
                    if self.nomes_modal.is_some() { return self.update(Msg::NomesFechar); }
                    if self.erro_explicando.is_some() { return self.update(Msg::ExplicacaoErroFechar); }
                    if self.link_modal.is_some() { return self.update(Msg::LinkFechar); }
                    if self.comentario_editor.is_some() { return self.update(Msg::Comentario(comentarios_ui::Evento::Cancelar)); }
                    if self.dimensao_modal.is_some() { return self.update(Msg::Dimensao(dimensao_dialogo::Evento::Fechar)); }
                    if self.celulas_modal.is_some() { return self.update(Msg::Celulas(celulas_dialogo::Evento::Fechar)); }
                    if self.subtotal.is_some() { return self.update(Msg::Topicos(topicos_ui::Evento::SubtotalFechar)); }
                    if self.protecao_modal.is_some() { return self.update(Msg::Protecao(protecao_ui::Evento::Fechar)); }
                    if self.novo_estilo.is_some() { return self.update(Msg::NovoEstilo(estilo_novo::Evento::Fechar)); }
                    if self.colar_especial.is_some() { return self.update(Msg::ColarEspecialJanela(colar_especial::Evento::Fechar)); }
                    if self.ir_para.is_some() { return self.update(Msg::IrParaJanela(ir_para::Evento::Fechar)); }
                    if self.classificar.is_some() { return self.update(Msg::Classificar(classificar::Evento::Fechar)); }
                    if self.atingir_meta.is_some() { return self.update(Msg::AtingirMeta(atingir_meta::Evento::Fechar)); }
                    if self.tabela_dados.is_some() { return self.update(Msg::TabelaDados(tabela_dados::Evento::Fechar)); }
                    if self.cenarios.is_some() { return self.update(Msg::Cenarios(cenarios_ui::Evento::Fechar)); }
                    if self.congelar_menu_aberto { return self.update(Msg::CongelarFechar); }
                    if self.avaliar.is_some() { return self.update(Msg::Avaliar(avaliar_ui::Evento::Fechar)); }
                    if self.criar_nomes.is_some() { return self.update(Msg::Formulas(formulas_ui::Evento::CriarFechar)); }
                    if self.formulas_menu.is_some() { return self.update(Msg::Formulas(formulas_ui::Evento::FecharMenu)); }
                    if self.condicional_aberto { return self.update(Msg::CondicionalFechar); }
                    if self.condicional_menu_aberto { return self.update(Msg::CondicionalAbrir); }
                    if self.efeitos_imagem.aberto { return self.update(Msg::ImagemLaboratorioFechar); }
                    if self.graficos.cor_ativa.is_some() {
                        return self.evento_grafico(graficos::Evento::SeletorCor(EventoCor::Fechar));
                    }
                    if self.graficos.seletor_aberto { return self.evento_grafico(graficos::Evento::FecharSeletor); }
                    if self.graficos.aberto { return self.evento_grafico(graficos::Evento::Fechar); }
                }
                // Teclas digitadas no modal são tratadas pelos text_inputs;
                // não deixar os atalhos da grade editar células por trás dele.
                if (self.estatisticas.is_some() || self.perfil_coluna.is_some())
                    && self.janelas.principal == Some(id) { return Task::none(); }
                // O campo Referência trata o Enter; as teclas não vão à grade.
                if self.ir_para.is_some() && self.janelas.principal == Some(id) {
                    return Task::none();
                }
                if self.classificar.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) =>
                            self.update(Msg::Classificar(classificar::Evento::Confirmar)),
                        _ => Task::none(),
                    };
                }
                if self.colar_especial.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) =>
                            self.update(Msg::ColarEspecialJanela(colar_especial::Evento::Confirmar)),
                        _ => Task::none(),
                    };
                }
                if self.avaliar.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) =>
                            self.update(Msg::Avaliar(avaliar_ui::Evento::Avaliar)),
                        _ => Task::none(),
                    };
                }
                if self.criar_nomes.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) =>
                            self.update(Msg::Formulas(formulas_ui::Evento::CriarConfirmar)),
                        _ => Task::none(),
                    };
                }
                if self.subtotal.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) => self.update(Msg::Topicos(topicos_ui::Evento::SubtotalAplicar)),
                        _ => Task::none(),
                    };
                }
                if self.celulas_modal.is_some() && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Enter) => self.update(Msg::Celulas(celulas_dialogo::Evento::Aplicar)),
                        _ => Task::none(),
                    };
                }
                if (self.link_modal.is_some() || self.comentario_editor.is_some() || self.dimensao_modal.is_some() || self.novo_estilo.is_some() || self.protecao_modal.is_some())
                    && self.janelas.principal == Some(id) {
                    return Task::none();
                }
                if self.editando_nome_aba && self.janelas.principal == Some(id) {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) => self.update(Msg::CancelarNomeAba),
                        keyboard::Key::Named(keyboard::key::Named::Enter) => self.salvar_nome_aba(),
                        _ => Task::none(),
                    };
                }
                if mods.control() || key == keyboard::Key::Named(keyboard::key::Named::Control) {
                    self.ctrl_pressionado = true;
                }
                if let Some(aux) = self.janelas.tipo(id) {
                    if aux == Auxiliar::Configuracoes {
                        if let Some(acao) = self.atalho_captura {
                            if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                                self.atalho_captura = None;
                                return Task::none();
                            }
                            if let Some(combo) = feather_ui::atalhos::combo_de(&key, mods) {
                                match configuracoes_ui::atalho_valido(self, acao, &combo) {
                                    Ok(()) => {
                                        self.config.atalhos.insert(acao.to_owned(), combo);
                                        self.config.gravar();
                                        self.atalho_captura = None;
                                        self.status = tr!("Atalho atualizado").into();
                                    }
                                    Err(erro) => self.status = erro.to_owned(),
                                }
                            }
                            return Task::none();
                        }
                    }
                    if aux == Auxiliar::Macros {
                        return self.atalho_macro(&key, mods).unwrap_or_else(Task::none);
                    }
                    // Ctrl+F dentro da Ajuda deve buscar no manual, não na grade.
                    if aux == Auxiliar::Ajuda && mods == keyboard::Modifiers::CTRL
                        && matches!(&key, keyboard::Key::Character(c)
                            if c.as_str().eq_ignore_ascii_case("f")) {
                        return feather_ui::ajuda::focar_busca::<Msg>();
                    }
                    use feather_ui::atalhos::PainelGlobal;
                    let painel = match aux {
                        Auxiliar::Ajuda => Some(PainelGlobal::Ajuda),
                        Auxiliar::Monitor => Some(PainelGlobal::Monitor),
                        Auxiliar::Configuracoes => Some(PainelGlobal::Configuracoes),
                        Auxiliar::FormatarCelulas => None,
                        Auxiliar::DividirTexto => None,
                        Auxiliar::RemoverDuplicados => None,
                        Auxiliar::Imprimir => None,
                        Auxiliar::OpenQuery => None,
                        Auxiliar::Inspecao => None,
                        Auxiliar::PrimeirosPassos | Auxiliar::Novidades => None,
                        // Outros painéis não respondem a F1/F12, somente a Esc.
                        Auxiliar::Macros => None,
                    };
                    // Esc fecha primeiro o assistente modal; só depois a janela.
                    if aux == Auxiliar::OpenQuery
                        && key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                        if self.fechar_assistente_openquery() { return Task::none(); }
                    }
                    let fechar = painel.is_some_and(|p|
                        feather_ui::atalhos::fechar_painel(p, &key, mods))
                        || (painel.is_none()
                            && key == keyboard::Key::Named(keyboard::key::Named::Escape));
                    if fechar {
                        if aux == Auxiliar::FormatarCelulas { self.formato_celula = None; }
                        if aux == Auxiliar::DividirTexto { self.assistente_divisao = None; }
                        if aux == Auxiliar::RemoverDuplicados { self.duplicados = None; }
                        if aux == Auxiliar::OpenQuery { self.openquery = None; }
                        // F1/F12 repetido logo depois de abrir não fecha (ver `alternancia_repetida`).
                        if painel.is_some() && key != keyboard::Key::Named(keyboard::key::Named::Escape)
                            && self.alternancia_repetida(aux) { return Task::none(); }
                        return self.janelas.fechar_tipo(aux);
                    }
                    return Task::none();
                }
                if self.contexto.is_some() && key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    return self.update(Msg::FecharContexto);
                }
                // Esc na grade tira a marca tracejada do Ctrl+C.
                if !self.menu_aberto && self.marca_copia.is_some()
                    && key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    self.marca_copia = None;
                    return Task::none();
                }
                if self.menu_aberto && !mods.control() && !mods.alt() {
                    return match key {
                        keyboard::Key::Named(keyboard::key::Named::Escape) =>
                            self.update(Msg::FecharMenu),
                        keyboard::Key::Named(keyboard::key::Named::Backspace) =>
                            self.update(Msg::MenuPagina(menu_principal::Pagina::Inicio)),
                        _ => Task::none(),
                    };
                }
                if self.barra_menu_aberta && !mods.control() && !mods.alt() {
                    use feather_ui::atalhos::AcaoGlobal;
                    match feather_ui::atalhos::acao_global(&key, mods) {
                        Some(AcaoGlobal::Ajuda) => return self.update(Msg::Ajuda),
                        Some(AcaoGlobal::Monitor) => return self.update(Msg::Monitor),
                        _ => {}
                    }
                    use keyboard::key::Named;
                    return match key {
                        keyboard::Key::Named(Named::Escape) => self.update(Msg::FecharMenu),
                        keyboard::Key::Named(Named::Backspace) |
                        keyboard::Key::Named(Named::ArrowLeft) => self.update(Msg::MenuVoltar),
                        keyboard::Key::Named(Named::ArrowDown) => self.update(Msg::MenuMover(1)),
                        keyboard::Key::Named(Named::ArrowUp) => self.update(Msg::MenuMover(-1)),
                        keyboard::Key::Named(Named::Enter) |
                        keyboard::Key::Named(Named::ArrowRight) => self.update(Msg::MenuEscolher(self.menu_cursor)),
                        _ => Task::none(),
                    };
                }
                // O TextEditor trata as teclas enquanto o modo multilinha está
                // aberto. Não executar navegação ou atalhos da grade em paralelo.
                if self.multilinha.is_some() { return Task::none(); }
                // Lista suspensa aberta: as teclas são dela (↑/↓, Enter, Esc).
                if self.lista_aberta && !self.editor_formula.ativo && self.janelas.principal == Some(id)
                    && !mods.control() && !mods.alt()
                {
                    if let Some(tarefa) = self.tecla_na_lista(&key) { return tarefa; }
                }
                if self.editor_formula.ativo && mods.alt()
                    && key == keyboard::Key::Named(keyboard::key::Named::Enter) {
                    return self.update(Msg::InserirQuebraLinha);
                }
                if self.editor_formula.ativo {
                    use keyboard::key::Named;
                    if key==keyboard::Key::Named(Named::Escape) {
                        if !self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US").is_empty() {
                            self.editor_formula.esconder();
                            return Task::none();
                        }
                        return self.update(Msg::Cancelar);
                    }
                    if self.edicao.starts_with('=') {
                        if key==keyboard::Key::Named(Named::F4) {
                            if self.editor_formula.alternar_referencia(&mut self.edicao) {
                                return Task::batch([
                                    text_input::focus(text_input::Id::new("feathersheet.fx")),
                                    text_input::move_cursor_to(text_input::Id::new("feathersheet.fx"),
                                        self.editor_formula.posicao(&self.edicao)),
                                ]);
                            }
                            return Task::none();
                        }
                        if key==keyboard::Key::Named(Named::F2) {
                            self.editor_formula.alternar_apontamento();
                            return Task::none();
                        }
                        let delta=match &key {
                            keyboard::Key::Named(Named::ArrowUp)=>Some((-1,0)),
                            keyboard::Key::Named(Named::ArrowDown)=>Some((1,0)),
                            keyboard::Key::Named(Named::ArrowLeft)=>Some((0,-1)),
                            keyboard::Key::Named(Named::ArrowRight)=>Some((0,1)),
                            _=>None,
                        };
                        if self.editor_formula.modo_texto {
                            let pos=self.editor_formula.posicao(&self.edicao);
                            if key==keyboard::Key::Named(Named::Home) {
                                self.editor_formula.posicionar(&self.edicao,0);
                            } else if key==keyboard::Key::Named(Named::End) {
                                self.editor_formula.posicionar(&self.edicao,self.edicao.chars().count());
                            } else if let Some((_,dc))=delta {
                                self.editor_formula.posicionar(&self.edicao,
                                    (pos as i64+dc as i64).clamp(0,self.edicao.chars().count() as i64) as usize);
                            }
                            return Task::none();
                        }
                        if let Some((dr,dc))=delta {
                            if !self.editor_formula.modo_texto &&
                                (self.editor_formula.pode_apontar(&self.edicao) || self.editor_formula.modo_apontar) {
                                let (r,c)=self.editor_formula.ponto.unwrap_or((self.linha,self.coluna));
                                let (nr,nc)=if mods.control() {
                                    self.destino_ctrl_seta_de((r,c),dr,dc)
                                } else {(
                                    (r as i64+dr as i64).clamp(0,(MAX_ROWS-1) as i64) as u32,
                                    (c as i64+dc as i64).clamp(0,(MAX_COLS-1) as i64) as u32
                                )};
                                let outra=self.editor_formula.origem.and_then(|(origem,_,_)|
                                    (origem!=self.folha).then(||self.workbook.nomes().get(self.folha).cloned()).flatten());
                                if self.editor_formula.referencia_na_aba(&mut self.edicao,nr,nc,mods.shift(),outra.as_deref()) {
                                    // Mesma conta da seleção: zoom e medidas reais.
                                    if self.mostrar_celula(nr, nc, crate::grade::Exigencia::Inteira).is_none() {
                                        if nr<self.linha_inicio || nr>=self.linha_inicio.saturating_add(self.config.linhas_visiveis) {
                                            self.linha_inicio=nr.saturating_sub(2);
                                        }
                                        if nc<self.coluna_inicio || nc>=self.coluna_inicio.saturating_add(self.config.colunas_visiveis) {
                                            self.coluna_inicio=nc.saturating_sub(2);
                                        }
                                    }
                                    return Task::batch([
                                        text_input::focus(text_input::Id::new("feathersheet.fx")),
                                        text_input::move_cursor_to(text_input::Id::new("feathersheet.fx"),
                            self.editor_formula.posicao(&self.edicao)),
                                    ]);
                                }
                            } else if dr!=0 && !self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US").is_empty() {
                                return self.update(Msg::NavegarSugestao(dr as i32));
                            } else if dc!=0 && !mods.control() && !mods.alt() {
                                let atual=self.editor_formula.posicao(&self.edicao);
                                self.editor_formula.posicionar(&self.edicao,
                                    (atual as i64+dc as i64)
                                        .clamp(0,self.edicao.chars().count() as i64) as usize);
                            }
                            return Task::none();
                        }
                    }
                    // Tab confirma e anda para a direita (Shift+Tab, para a
                    // esquerda), como no Excel; antes descia como o Enter.
                    // Com a lista de funções aberta, aceita a sugestão.
                    if key==keyboard::Key::Named(Named::Tab) {
                        if !self.editor_formula.sugestoes(&self.edicao,self.config.idioma=="en-US").is_empty() {
                            return self.update(Msg::AceitarSugestao(self.editor_formula.sugestao));
                        }
                        if !self.confirmar() { return Task::none(); }
                        let navegacao=self.update(Msg::Mover(0,if mods.shift() {-1} else {1}));
                        return Task::batch([navegacao,crate::campo_fx::desfocar()]);
                    }
                    // Campo fx: teclas Home/End atualizam o índice usado pelo
                    // assistente, mantendo o controle visual do Iced.
                    if key==keyboard::Key::Named(Named::Home) {
                        self.editor_formula.posicionar(&self.edicao,0);
                    } else if key==keyboard::Key::Named(Named::End) {
                        self.editor_formula.posicionar(&self.edicao,self.edicao.chars().count());
                    }
                    return Task::none();
                }
                if key == keyboard::Key::Named(keyboard::key::Named::Escape) {
                    // Sem nenhum painel, menu ou modo aberto, Esc recolhe a
                    // seleção de intervalo para a célula ativa.
                    let livre = self.edicao_menu.is_none()
                        && !self.tabela_popup_filtro
                        && self.faixa_ativa != AbaFaixa::Tabela
                        && self.tabela_area.is_none()
                        && self.arraste_imagem.is_none()
                        && self.imagem_selecionada.is_none()
                        && self.pincel_formatacao.is_none()
                        && !self.busca.aberta
                        && self.contexto.is_none()
                        && self.pendente.is_none()
                        && !self.menu_aberto
                        && !self.editando_nome_aba;
                    if livre && self.fim_selecao != (self.linha, self.coluna) {
                        return self.update(Msg::Selecionar(self.linha, self.coluna));
                    }
                    return self.update(Msg::Cancelar);
                }
                if let Some(acao) = feather_ui::atalhos::acao_global(&key, mods) {
                    use feather_ui::atalhos::AcaoGlobal;
                    return self.update(match acao {
                        AcaoGlobal::Ajuda => Msg::Ajuda,
                        AcaoGlobal::Monitor => Msg::Monitor,
                        AcaoGlobal::AlternarFaixa => Msg::Faixa,
                    });
                }
                if let Some(atalho) = feather_ui::atalhos::combo_de(&key, mods) {
                    if let Some(acao) = configuracoes_ui::comando_do_atalho(self, &atalho) {
                        return self.update(acao);
                    }
                    // Navegação de guias usa somente as visíveis (as ocultas
                    // continuam no arquivo e não devem capturar a seleção).
                    if matches!(atalho.as_str(), "Ctrl+PageDown" | "Ctrl+PageUp"
                        | "Alt+Shift+PageDown" | "Alt+Shift+PageUp")
                    {
                        let visiveis: Vec<usize> = self.workbook.abas_visiveis().collect();
                        let atual = visiveis.iter().position(|&i| i == self.folha);
                        if let Some(pos) = atual {
                            let para_direita = atalho.ends_with("PageDown");
                            let vizinha = if para_direita {
                                visiveis.get(pos + 1).copied()
                            } else { pos.checked_sub(1).map(|p| visiveis[p]) };
                            if let Some(destino) = vizinha {
                                return if atalho.starts_with("Alt+Shift+") {
                                    self.update(Msg::MoverAba(self.folha, destino))
                                } else { self.update(Msg::Aba(destino)) };
                            }
                        }
                        return Task::none();
                    }
                    let outro = match atalho.as_str() {
                        "Ctrl+N" => Some(Msg::Novo), "Ctrl+O" => Some(Msg::Abrir),
                        "Ctrl+P" => Some(Msg::Impressao(EventoImpressao::Abrir)),
                        "Ctrl+C" => Some(Msg::Copiar), "Ctrl+X" => Some(Msg::Recortar),
                        "Ctrl+V" => Some(Msg::ColarSolicitar),
                        "Ctrl+Alt+V" => Some(Msg::ColarEspecialJanela(colar_especial::Evento::Abrir)),
                        "F5" | "Ctrl+G" => Some(Msg::IrParaJanela(ir_para::Evento::Abrir)),
                        "Ctrl+Z" => Some(Msg::Desfazer), "Ctrl+Y" | "Ctrl+Shift+Z" => Some(Msg::Refazer),
                        "Ctrl+F" => Some(Msg::BuscaAba(false)),
                        "Ctrl+H" => Some(Msg::BuscaAba(true)),
                        // Mesmos atalhos dos comandos Preencher do Excel/TypeScript;
                        // somente quando a edição de fórmula está encerrada.
                        "Ctrl+D" => Some(Msg::Preencher(DirecaoPreenchimento::Baixo)),
                        "Ctrl+E" => Some(Msg::PreenchimentoRelampago),
                        // Inserir/Excluir células do Excel (Ctrl++ no teclado numérico ou Ctrl+Shift+=).
                        // Agrupar/Desagrupar do Excel (Dados → Estrutura de tópicos).
                        "Alt+Shift+ArrowRight" => Some(Msg::Topicos(topicos_ui::Evento::Agrupar)),
                        "Alt+Shift+ArrowLeft" => Some(Msg::Topicos(topicos_ui::Evento::Desagrupar)),
                        "Ctrl++" | "Ctrl+Shift++" | "Ctrl+Shift+=" => Some(Msg::Celulas(celulas_dialogo::Evento::Abrir(true))),
                        "Ctrl+-" => Some(Msg::Celulas(celulas_dialogo::Evento::Abrir(false))),
                        "Ctrl+R" => Some(Msg::Preencher(DirecaoPreenchimento::Direita)),
                        "Ctrl+A" => Some(Msg::SelecionarTudo),
                        "Ctrl+Shift+8" | "Ctrl+Shift+*" | "Ctrl+*" => Some(Msg::SelecionarRegiaoAtual),
                        "Ctrl+B" => Some(Msg::AlternarEstilo("bold".into())),
                        "Ctrl+I" => Some(Msg::AlternarEstilo("italic".into())),
                        "Ctrl+U" => Some(Msg::AlternarEstilo("underline".into())),
                        // Recuo do Excel: Ctrl+Alt+Tab aumenta, Ctrl+Alt+Shift+Tab diminui.
                        "Ctrl+Alt+Tab" => Some(Msg::AlterarRecuo(1)),
                        "Ctrl+Alt+Shift+Tab" => Some(Msg::AlterarRecuo(-1)),
                        "Ctrl+Shift+M" => Some(Msg::Monitor),
                        "Alt+=" => Some(Msg::AutoSoma),
                        "Ctrl+Shift+$" | "Ctrl+Shift+4" =>
                            Some(Msg::EstiloMenu("format".into(), Campo::Texto("currency".into()))),
                        "Ctrl+Shift+%" | "Ctrl+Shift+5" =>
                            Some(Msg::EstiloMenu("format".into(), Campo::Texto("percent".into()))),
                        "Shift+F11" => Some(Msg::AdicionarAba),
                        "Shift+Space" => Some(Msg::SelecionarLinha(self.linha)),
                        "Ctrl+Space" => Some(Msg::SelecionarColuna(self.coluna)),
                        "Alt+F11" => Some(Msg::Macros),
                        "F11" => Some(Msg::TelaInteira),
                        "F9" => Some(Msg::Formulas(formulas_ui::Evento::CalcularAgora)),
                        "Shift+F9" => Some(Msg::Formulas(formulas_ui::Evento::CalcularPlanilha)),
                        // Seleciona a matriz derramada da célula ativa (Excel: Ctrl+/).
                        "Ctrl+/" | "Ctrl+Shift+/" => Some(Msg::Formulas(formulas_ui::Evento::SelecionarDerramada)),
                        _ => None,
                    };
                    if let Some(outro) = outro { return self.update(outro); }
                }
                if self.pendente.is_none() && mods.alt() && !mods.control()
                    && key == keyboard::Key::Named(keyboard::key::Named::ArrowDown) {
                    return self.update(Msg::ListaDaColuna);
                }
                if self.pendente.is_none() {
                    let ctrl_delta = match &key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some((1, 0)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some((-1, 0)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some((0, 1)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some((0, -1)),
                        _ => None,
                    };
                    if mods.control() {
                        if let Some((dr, dc)) = ctrl_delta {
                            let destino = self.destino_ctrl_seta(dr, dc, mods.shift());
                            return self.update(if mods.shift() { Msg::Estender(destino.0, destino.1) } else {
                                Msg::Mover(destino.0 as i32 - self.linha as i32, destino.1 as i32 - self.coluna as i32)
                            });
                        }
                    }
                    if mods.shift() {
                        let delta = match &key {
                            keyboard::Key::Named(keyboard::key::Named::ArrowDown) => Some((1,0)),
                            keyboard::Key::Named(keyboard::key::Named::ArrowUp) => Some((-1,0)),
                            keyboard::Key::Named(keyboard::key::Named::ArrowRight) => Some((0,1)),
                            keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => Some((0,-1)),
                            _ => None,
                        };
                        if let Some((dr,dc)) = delta {
                            return self.update(Msg::Estender(
                                (self.fim_selecao.0 as i64 + dr).clamp(0,(MAX_ROWS-1) as i64) as u32,
                                (self.fim_selecao.1 as i64 + dc).clamp(0,(MAX_COLS-1) as i64) as u32));
                        }
                    }
                    match key {
                        keyboard::Key::Named(keyboard::key::Named::ArrowDown) => return self.update(Msg::Mover(1,0)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowUp) => return self.update(Msg::Mover(-1,0)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowRight) => return self.update(Msg::Mover(0,1)),
                        keyboard::Key::Named(keyboard::key::Named::ArrowLeft) => return self.update(Msg::Mover(0,-1)),
                        keyboard::Key::Named(keyboard::key::Named::Enter) => return self.update(Msg::Mover(if mods.shift() {-1} else {1},0)),
                        keyboard::Key::Named(keyboard::key::Named::Tab) => return self.update(Msg::Mover(0,if mods.shift() {-1} else {1})),
                        keyboard::Key::Named(keyboard::key::Named::PageDown) => return self.update(if mods.alt() { Msg::ColunaPagina(1) } else { Msg::LinhaPagina(1) }),
                        keyboard::Key::Named(keyboard::key::Named::PageUp) => return self.update(if mods.alt() { Msg::ColunaPagina(-1) } else { Msg::LinhaPagina(-1) }),
                        keyboard::Key::Named(keyboard::key::Named::Delete) | keyboard::Key::Named(keyboard::key::Named::Backspace)
                            if self.botao_macro_selecionado.is_some() => {
                            self.apagar_botao_macro_selecionado();
                            return Task::none();
                        }
                        keyboard::Key::Named(keyboard::key::Named::Delete) | keyboard::Key::Named(keyboard::key::Named::Backspace)
                            if self.imagem_selecionada.is_some() => {
                            self.apagar_imagem_selecionada();
                            return Task::none();
                        }
                        keyboard::Key::Named(keyboard::key::Named::Delete) | keyboard::Key::Named(keyboard::key::Named::Backspace) => return self.update(Msg::Limpar),
                        keyboard::Key::Named(keyboard::key::Named::F2) => {
                            return iced::widget::text_input::focus(iced::widget::text_input::Id::new("feathersheet.fx"));
                        }
                        // Espaço com caixas de seleção na seleção: marca/desmarca (Excel).
                        _ if (matches!(&key, keyboard::Key::Named(keyboard::key::Named::Space))
                                || matches!(&key, keyboard::Key::Character(t) if t.as_str() == " "))
                            && !mods.control() && !mods.alt() && !mods.logo()
                            && self.recusa_antes(&Msg::Digitar(" ".into())).is_none()
                            && self.alternar_caixas_da_selecao() => {
                            return Task::none();
                        }
                        keyboard::Key::Character(t) if !mods.control() && !mods.alt() && !mods.logo() && !t.is_empty() => {
                            return self.update(Msg::Digitar(t.to_string()));
                        }
                        _ => {}
                    }
                }
                Task::none()
                }
}
