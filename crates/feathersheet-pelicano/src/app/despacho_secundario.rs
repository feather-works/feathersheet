//! Segunda parte do despacho de eventos: formatação, menus, janelas e configurações.
//! Recebe apenas variantes não tratadas pelo dispatcher principal.
use super::*;

impl App {
    pub(super) fn processar_mensagem_secundaria(&mut self, msg: Msg) -> Task<Msg> {
        match msg {
            Msg::CongelarMenu => {
                self.congelar_menu_aberto = !self.congelar_menu_aberto;
                self.congelar_popup = (self.ponteiro_x - 20., self.ponteiro_y + 24.);
                self.menu_aberto = false;
                Task::none()
            }
            Msg::CongelarFechar => { self.congelar_menu_aberto = false; Task::none() }
            Msg::Desenhar(evento) => self.evento_desenhar(evento),
            Msg::Tinta(evento) => self.evento_tinta(evento),
            Msg::Objetos(evento) => self.evento_objetos(evento),
            Msg::Caneta(ajuste) => self.ajustar_caneta(ajuste),
            Msg::CongelarAplicar(acao) => self.aplicar_congelamento(acao),
            Msg::LinkEditar => self.editar_link(),
            Msg::LinkFechar => { self.link_modal = None; Task::none() }
            Msg::Dimensao(evento) => self.evento_dimensao(evento),
            Msg::Celulas(evento) => self.evento_celulas(evento),
            Msg::Topicos(evento) => self.evento_topicos(evento),
            Msg::VbaImportar => self.importar_vba(),
            Msg::MacroResponder(sim) => { self.responder_macro(sim); Task::none() }
            Msg::MacroMensagemOk => {
                if !self.macro_mensagens.is_empty() { self.macro_mensagens.remove(0); }
                Task::none()
            }
            Msg::Protecao(evento) => self.evento_protecao(evento),
            Msg::Comentario(evento) => self.evento_comentario(evento),
            Msg::VbaLido(lido) => { self.vba_lido(lido); Task::none() }
            Msg::NovoEstilo(evento) => self.evento_novo_estilo(evento),
            Msg::ColarEspecialJanela(evento) => self.evento_colar_especial(evento),
            Msg::SelecionarAdicional(r, c) => { self.selecionar_adicional(r, c); Task::none() }
            Msg::IrParaJanela(evento) => self.evento_ir_para(evento),
            Msg::Classificar(evento) => self.evento_classificar(evento),
            Msg::AtingirMeta(evento) => self.evento_atingir_meta(evento),
            Msg::TabelaDados(evento) => self.evento_tabela_dados(evento),
            Msg::Cenarios(evento) => self.evento_cenarios(evento),
            Msg::MesclarModo(modo) => { self.mesclar_modo(modo); Task::none() }
            Msg::AjustarAltura => { self.ajustar_altura_da_selecao(); Task::none() }
            Msg::LinkEndereco(valor) => {
                if let Some(s) = &mut self.link_modal { s.endereco = valor; s.erro = None; }
                Task::none()
            }
            Msg::LinkTexto(valor) => {
                if let Some(s) = &mut self.link_modal {
                    if !s.formula { s.texto = valor; }
                }
                Task::none()
            }
            Msg::LinkSalvar => self.salvar_link(),
            Msg::LinkRemover => self.remover_link(),
            Msg::LinkAbrir => self.abrir_link(),
            Msg::LinkAbrirEm(linha, coluna) => {
                if !self.confirmar() { return Task::none(); }
                self.trocar_selecao(linha, coluna);
                self.abrir_link()
            },
            Msg::NomesAbrir(preencher) => self.abrir_nomes(preencher),
            Msg::NomesFechar => { self.nomes_modal = None; Task::none() }
            Msg::NomeAtualizar(nome) => {
                if let Some(e) = &mut self.nomes_modal {
                    e.nome = nome;
                    e.erro = None;
                }
                Task::none()
            }
            Msg::NomeCriar => self.criar_nome(),
            Msg::NomeRemover(nome) => self.excluir_nome(&nome),
            Msg::NomeIrPara(nome) => self.ir_para_nome(&nome),
            Msg::NomesEvento(evento) => self.evento_nomes(evento),
            Msg::Dinamica(evento) => self.evento_dinamica(evento),
            Msg::Vinculos(evento) => self.evento_vinculos(evento),
            Msg::CondicionalAbrir => {
                self.bordas_menu_aberto = false;
                self.tabelas_menu_aberto = false;
                self.estilos_menu_aberto = false;
                self.condicional_menu_aberto = !self.condicional_menu_aberto;
                self.condicional_aberto = false;
                if self.condicional_menu_aberto {
                    // Guardar a âncora do clique, e não a largura fixa do menu.
                    // A geometria responsiva fica em condicional_ui.rs: em janelas
                    // estreitas, o popup diminui sem saltar para a esquerda.
                    self.condicional_menu_x = (self.ponteiro_x - 35.0).max(8.0);
                    self.condicional_menu_y = (self.ponteiro_y + 36.0).max(128.0);
                }
                Task::none()
            }
            Msg::CondicionalMostrar => {
                self.condicional_menu_aberto = false;
                self.condicional_aberto = true;
                self.condicional_editando = None;
                self.condicional_intervalo = endereco_area(&self.area_selecionada());
                Task::none()
            }
            Msg::CondicionalFechar => {
                self.condicional_cor_aberta = None;
                self.condicional_menu_aberto = false;
                self.condicional_aberto = false;
                self.condicional_pegando = false;
                self.condicional_editando = None;
                Task::none()
            }
            Msg::CondicionalNovaRegra => {
                self.condicional_menu_aberto = false;
                self.condicional_aberto = true;
                self.condicional_editando = None;
                self.condicional_intervalo = endereco_area(&self.area_selecionada());
                self.condicional_operador = TipoCondicional::Maior;
                self.condicional_valor = "10".into();
                self.condicional_valor2 = "20".into();
                Task::none()
            }
            Msg::CondicionalPredefinida(tipo) => {
                self.condicional_menu_aberto = false;
                self.condicional_aberto = true;
                self.condicional_editando = None;
                self.condicional_intervalo = endereco_area(&self.area_selecionada());
                self.condicional_operador = tipo;
                self.condicional_valor = match tipo {
                    TipoCondicional::Formula => "=A1>10".into(),
                    TipoCondicional::Contem | TipoCondicional::NaoContem => String::new(),
                    _ => "10".into(),
                };
                self.condicional_valor2 = "20".into();
                Task::none()
            }
            Msg::CondicionalTipo(tipo) => {self.condicional_operador=tipo;self.condicional_cor_aberta=None;Task::none()}
            Msg::CondicionalCorAbrir(campo) => {self.abrir_cor_condicional(campo);Task::none()}
            Msg::CondicionalSeletorCor(evento) => self.evento_cor_condicional(evento),
            Msg::CondicionalValor(valor) => {self.condicional_valor=valor;Task::none()}
            Msg::CondicionalValor2(valor) => {self.condicional_valor2=valor;Task::none()}
            Msg::CondicionalIntervalo(valor) => {
                self.condicional_intervalo = valor;
                self.condicional_pegando = false;
                Task::none()
            }
            Msg::CondicionalPegar => {
                self.condicional_pegando = !self.condicional_pegando;
                if self.condicional_pegando {
                    self.status = tr!("Selecione na planilha a área da regra.").into();
                }
                Task::none()
            }
            Msg::CondicionalSoSelecao(sim) => { self.condicional_so_selecao = sim; Task::none() }
            Msg::CondicionalCor(cor) => {self.condicional_cor=cor;Task::none()}
            Msg::CondicionalCorTexto(cor) => {self.condicional_cor_texto=cor;Task::none()}
            Msg::CondicionalCorMin(cor) => {self.condicional_cor_min=cor;Task::none()}
            Msg::CondicionalCorMeio(cor) => {self.condicional_cor_meio=cor;Task::none()}
            Msg::CondicionalCorNegativa(cor) => {self.condicional_cor_negativa=cor;Task::none()}
            Msg::CondicionalPontoMeio(b) => {self.condicional_ponto_meio=b;Task::none()}
            Msg::CondicionalOcultarValor(b) => {self.condicional_ocultar_valor=b;Task::none()}
            Msg::CondicionalNegrito(b) => {self.condicional_negrito=b;Task::none()}
            Msg::CondicionalItalico(b) => {self.condicional_italico=b;Task::none()}
            Msg::CondicionalSublinhado(b) => {self.condicional_sublinhado=b;Task::none()}
            Msg::CondicionalRiscado(b) => {self.condicional_riscado=b;Task::none()}
            Msg::CondicionalAdicionar => {
                self.adicionar_condicional(false);Task::none()
            }
            Msg::CondicionalCoresAlternadas => {
                self.adicionar_condicional(true);Task::none()
            }
            Msg::CondicionalEditar(i) => {
                self.editar_condicional(i);
                Task::none()
            }
            Msg::CondicionalRemover(i) => {
                let mut r=condicional::regras(&self.workbook,self.folha);
                self.condicional_editando = None;
                if i < r.len() {r.remove(i);self.gravar_condicionais(r);}
                Task::none()
            }
            Msg::CondicionalMover(i,d) => {
                let mut r=condicional::regras(&self.workbook,self.folha);
                self.condicional_editando = None;
                let j=(i as i64)+(d as i64);
                if i < r.len() && j >= 0 && (j as usize) < r.len() {
                    r.swap(i,j as usize);self.gravar_condicionais(r);
                }
                Task::none()
            }
            Msg::CondicionalLimparSelecao => {
                self.condicional_menu_aberto = false;
                self.condicional_editando = None;
                let area = self.area_selecionada();
                match condicional::limpar_selecao(&mut self.workbook,
                    self.folha,area) {
                    Ok(_) => {self.alterado=true;self.status=tr!("Regras da seleção removidas").into();}
                    Err(e) => self.status=e,
                }
                Task::none()
            }
            Msg::CondicionalLimparTudo => {
                self.condicional_menu_aberto = false;
                self.condicional_editando = None;
                self.gravar_condicionais(Vec::new());Task::none()
            }
            Msg::Formatar(acao) => { self.formatar(acao); Task::none() }
            Msg::EstiloMenu(chave, valor) => { self.aplicar_estilo_menu(&chave, valor); Task::none() }
            Msg::AlternarEstilo(chave) => { self.alternar_estilo_menu(&chave); Task::none() }
            Msg::InserirMenu(acao, depois) => { self.inserir_menu(acao, depois); Task::none() }
            Msg::AbrirSeletorCor(alvo) => {
                self.bordas_menu_aberto = false;
                self.orientacao_menu_aberto = false;
                if self.cor_ativa == Some(alvo) {
                    self.cor_ativa = None;
                    return Task::none();
                }
                let atual = self.cor_da_celula(alvo);
                let rgba = if atual.is_empty() {
                    [0, 0, 0, 255]
                } else { seletor_cor::rgba_de(&atual) };
                self.cor_hsva = seletor_cor::rgba_para_hsva(rgba, self.cor_hsva[0]);
                self.cor_hex = if atual.is_empty() { "#000000".into() } else { atual };
                self.cor_ativa = Some(alvo);
                Task::none()
            }
            Msg::SeletorCor(evento) => self.atualizar_seletor_cor(evento),
            Msg::FaixaOpcao(grupo, opcao) => self.faixa_escolha(grupo, opcao),
            Msg::BuscaComandos(consulta) => { self.busca_comandos = consulta; Task::none() }
            Msg::BuscaComando(i) => {
                let acao = self.resultados_faixa().get(i).map(|(_,acao)|acao.clone());
                self.busca_comandos.clear();
                if let Some(msg) = acao.as_ref().and_then(|a| self.evento_menu(a)) {
                    self.update(msg)
                } else { Task::none() }
            }
            Msg::TamanhoFonteDelta(delta) => {
                let atual = self.workbook.aba(self.folha)
                    .and_then(|aba|aba.sheet.estilo_resolvido(self.linha,self.coluna))
                    .and_then(|s|s.font_size).unwrap_or(13.).round() as i32;
                self.update(Msg::EstiloMenu("fontSize".into(),
                    Campo::Numero((atual + delta).clamp(6, 96) as f64)))
            }
            Msg::Decimais(delta) => {
                let atual = self.workbook.aba(self.folha)
                    .and_then(|aba|aba.sheet.estilo_resolvido(self.linha,self.coluna))
                    .and_then(|s|s.decimals).unwrap_or(2);
                self.update(Msg::EstiloMenu("decimals".into(),
                    Campo::Numero((atual as i32 + delta as i32).clamp(0,15) as f64)))
            }
            Msg::AlterarRecuo(delta) => {
                self.confirmar();
                let area = self.area_selecionada();
                let mut mudou = false;
                self.workbook.editar(self.folha, |sheet| {
                    mudou = feathersheet_core::operacoes::recuo::alterar_recuo(sheet, &area, delta);
                });
                if mudou {
                    self.alterado = true;
                    let nivel = self.workbook.aba(self.folha).map_or(0, |a|
                        feathersheet_core::operacoes::recuo::recuo_de(&a.sheet, self.linha, self.coluna));
                    self.status = trf!("Recuo: nível {nivel}");
                } else {
                    self.status = if delta > 0 { tr!("Recuo já está no máximo (15)") } else { tr!("Recuo já está em 0") }.into();
                }
                Task::none()
            }
            Msg::LimparTudo => {
                self.estilos_menu_aberto = false;
                self.confirmar();
                let areas = self.areas_selecionadas();
                let sem_caixas = self.workbook.aba(self.folha)
                    .and_then(|a| crate::caixas::Regras::da(&a.sheet).sem_caixas_nas(&areas));
                let principal = self.area_selecionada();
                self.workbook.editar(self.folha, |sheet| {
                    sheet.abrir_transacao();
                    for area in &areas { sheet.clear(area); sheet.limpar_comentarios(area); }
                    if let Some(json) = sem_caixas { sheet.alterar_validacoes(Some(json), &principal); }
                    sheet.fechar_transacao();
                });
                self.alterado = true;
                self.edicao.clear();
                self.status = tr!("Conteúdo, formatos e comentários da seleção removidos").into();
                Task::none()
            }
            Msg::LimparFormato => {
                self.estilos_menu_aberto = false;
                self.confirmar();
                let area = self.area_selecionada();
                let mut patch = EstiloParcial::new();
                for chave in ["bold", "italic", "underline", "strike", "wrap", "fontSize",
                    "fontFamily", "decimals", "borders", "align", "vAlign",
                    "format", "color", "bg", "rotation", "link", "indent", "textVertical", "shrinkToFit"] {
                    patch.insert(chave.into(), Campo::Nulo);
                }
                // As caixas de seleção também são formato (Excel): saem da seleção.
                let sem_caixas = self.workbook.aba(self.folha)
                    .and_then(|a| crate::caixas::Regras::da(&a.sheet).sem_caixas_em(&area));
                self.workbook.editar(self.folha, |sheet| {
                    sheet.abrir_transacao();
                    sheet.set_style(&area, &patch);
                    if let Some(json) = sem_caixas { sheet.alterar_validacoes(Some(json), &area); }
                    sheet.fechar_transacao();
                });
                self.alterado = true;
                self.status = tr!("Formatação da seleção removida").into();
                Task::none()
            }
            Msg::AjustarLargura => {
                self.ajustar_largura_da_selecao();
                Task::none()
            }
            Msg::AjustarDivisoria(coluna, indice) => {
                self.ajustar_pela_divisoria(coluna, indice);
                Task::none()
            }
            Msg::DimensaoArrastada(coluna, indice, px) => {
                self.aplicar_dimensao_arrastada(coluna, indice, px);
                Task::none()
            }
            Msg::EstatisticasAbrir => self.abrir_estatisticas(),
            Msg::EstatisticasFechar => {
                self.estatisticas_geracao = self.estatisticas_geracao.wrapping_add(1);
                self.estatisticas = None;
                Task::none()
            }
            Msg::EstatisticasProntas(geracao, resultado) => {
                if geracao == self.estatisticas_geracao
                    && matches!(&self.estatisticas, Some(estatisticas_ui::Estado::Calculando { .. }))
                {
                    match resultado {
                        Ok(visao) => {
                            self.status = trf!("Estatísticas: {} valores preenchidos", visao.dados.count);
                            self.estatisticas = Some(estatisticas_ui::Estado::Prontas(visao));
                        }
                        Err(erro) => {
                            self.status = trf!("Erro ao calcular estatísticas: {erro}");
                            self.estatisticas = None;
                        }
                    }
                }
                Task::none()
            }
            Msg::PerfilColunaAbrir => self.abrir_perfil_coluna(),
            Msg::PerfilColunaPronto(geracao, resultado) => {
                if geracao == self.perfil_geracao {
                    match resultado {
                        Ok(visao) => {
                            self.status = trf!("Perfil: {} linhas analisadas", visao.dados.linhas);
                            self.perfil_coluna = Some(visao);
                        }
                        Err(e) => self.status = trf!("Erro no perfil: {e}"),
                    }
                }
                Task::none()
            }
            Msg::PerfilColunaFechar => {
                self.perfil_geracao = self.perfil_geracao.wrapping_add(1);
                self.perfil_coluna = None;
                Task::none()
            }
            Msg::PerfilColunaIrPara(linha) => {
                if let Some(perfil) = self.perfil_coluna.take() {
                    self.trocar_selecao(linha, perfil.coluna);
                }
                Task::none()
            }
            Msg::MostrarSobre => { self.config_secao = configuracoes_ui::SECAO_SOBRE; self.alternar(Auxiliar::Configuracoes) }
            Msg::AlternarMostrarFormulas => {
                // Equivale a toggleShowFormulas no TypeScript. Não modifica
                // Workbook, flags de arquivo alterado nem histórico de desfazer.
                self.mostrar_formulas = !self.mostrar_formulas;
                self.config.mostrar_formulas = self.mostrar_formulas;
                self.config.gravar();
                self.status = if self.mostrar_formulas {
                    tr!("Exibição de fórmulas ativada (Ctrl+`)")
                } else {
                    tr!("Exibição de resultados ativada (Ctrl+`)")
                }.into();
                Task::none()
            }
            Msg::AlternarBarraFx => {
                if self.editor_formula.ativo {
                    self.status = tr!("Conclua a edição antes de ocultar a barra de fórmulas").into();
                } else {
                    self.mostrar_fx = !self.mostrar_fx;
                    self.config.barra_formulas = self.mostrar_fx;
                    self.config.gravar();
                }
                Task::none()
            }
            Msg::MostrarCatalogoFuncoes => {
                self.selecionar_faixa(AbaFaixa::Inserir);
                if self.config.faixa_recolhida {
                    self.config.faixa_recolhida = false;
                    self.config.gravar();
                }
                Task::none()
            }
            Msg::Formulas(evento) => self.evento_formulas(evento),
            Msg::Inspecao(evento) => self.evento_inspecao(evento),
            Msg::Avaliar(evento) => self.evento_avaliar(evento),
            Msg::Mcp(pedido) => self.atender_mcp(pedido),
            Msg::McpImagem(pedido, tela) => self.responder_imagem(pedido, tela),
            Msg::InserirFuncao(nome) => {
                self.formulas_menu = None;
                self.lembrar_funcao(&nome);
                if self.editor_formula.ativo && self.edicao.starts_with('=') {
                    let posicao = self.editor_formula.posicao(&self.edicao);
                    let indice = self.edicao.char_indices().nth(posicao)
                        .map(|(b, _)| b).unwrap_or(self.edicao.len());
                    let mut nova = self.edicao.clone();
                    nova.insert_str(indice, &format!("{nome}("));
                    let edicao = self.update(Msg::Editar(nova));
                    // shortcut: o text_input do pelicano não recebe a posição do cursor por Task
                    // (ia para depois do "nome("); fica onde estava. Melhorar quando houver mover cursor por id.
                    let _ = posicao;
                    Task::batch([edicao, Task::focus("feathersheet.fx")])
                } else {
                    self.update(Msg::Digitar(format!("={nome}(")))
                }
            }
            Msg::Estrutura(acao) => { self.estrutura(acao); Task::none() }
            Msg::LinhasGrade(v) => {
                // Aba que veio do Excel sem grade: ligar mostra a grade nela,
                // sem mexer na preferência global (se já estava ligada).
                if let Some(id) = self.workbook.aba(self.folha).map(|a| a.id) {
                    if v && self.grade_oculta.remove(&id) && self.linhas_grade {
                        return Task::none();
                    }
                }
                self.linhas_grade = v;
                self.config.linhas_grade = v;
                self.config.gravar();
                Task::none()
            }
            Msg::RealceEixos(v) => {
                self.config.realce_eixos = v;
                self.config.gravar();
                Task::none()
            }
            Msg::Mover(dr,dc) => {
                if !self.confirmar() { return Task::none(); }
                let (linha, coluna) = self.passo_visivel((self.linha, self.coluna), dr as i64, dc as i64);
                self.trocar_selecao(linha, coluna);
                Task::none()
            }
            Msg::LinhaPagina(n) => {
                let (linhas_fixas, _) = self.congelamento_atual();
                let pagina = self.pagina_da_grade().0;
                let inicio = (self.linha_inicio as i64 + n as i64 * pagina as i64)
                    .clamp(linhas_fixas as i64,
                        (MAX_ROWS - pagina).max(linhas_fixas) as i64) as u32;
                self.linha_inicio = inicio;
                Task::none()
            }
            Msg::ColunaPagina(n) => {
                let (_, colunas_fixas) = self.congelamento_atual();
                let pagina = self.pagina_da_grade().1;
                self.coluna_inicio = (self.coluna_inicio as i64 + n as i64 * pagina as i64)
                    .clamp(colunas_fixas as i64,
                        (MAX_COLS - pagina).max(colunas_fixas) as i64) as u32;
                Task::none()
            }
            Msg::Aba(i) => {
                // Edição inline: clicar em outra guia confirma o nome atual.
                // Se inválido, mantém o foco na guia e não perde o rascunho.
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                let nova=i.min(self.workbook.len().saturating_sub(1));
                if self.workbook.aba(nova).is_some_and(|a| a.sheet.oculta) {
                    return Task::none();
                }
                if self.folha != nova {
                    self.botao_macro_selecionado = None;
                    self.arraste_botao_macro = None;
                }
                if self.editor_formula.ativo && self.edicao.starts_with('=') {
                    // A célula de origem continua em EditorFormula::origem.
                    // Navegar nas abas não pode gravar uma fórmula pela metade.
                    self.folha=nova;
                    let (r,c)=self.editor_formula.origem
                        .filter(|(origem,_,_)|*origem==nova)
                        .map(|(_,r,c)|(r,c)).unwrap_or((0,0));
                    self.linha=r;
                    self.coluna=c;
                    self.fim_selecao=(r,c);
                    let (fixas_linhas,fixas_colunas)=self.congelamento_atual();
                    self.linha_inicio=r.saturating_sub(2).max(fixas_linhas);
                    self.coluna_inicio=c.saturating_sub(2).max(fixas_colunas);
                    self.endereco=cell_name(r,c);
                    // Trocar de aba não deve estender um intervalo iniciado na anterior.
                    self.editor_formula.ponto=None;
                    self.editor_formula.ancora=None;
                    self.nome_aba=self.workbook.nomes().get(nova).cloned().unwrap_or_default();
                    return Task::none();
                }
                if !self.confirmar() { return Task::none(); }
                self.folha=nova;
                // A origem rolável da aba anterior não deve contaminar
                // o painel congelado da nova aba.
                let (fixas_linhas,fixas_colunas)=self.congelamento_atual();
                self.linha_inicio=fixas_linhas;
                self.coluna_inicio=fixas_colunas;
                self.trocar_selecao(0,0);
                self.editando_nome_aba=false;
                self.nome_aba=self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
                Task::none()
            }
            Msg::AdicionarAba => {
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                let mantendo_formula=self.editor_formula.ativo && self.edicao.starts_with('=');
                if !mantendo_formula {self.confirmar();}
                let nomes = self.workbook.nomes();
                let mut n = 1;
                let prefixo = tr!("Planilha");
                while nomes.iter().any(|nome| nome.eq_ignore_ascii_case(&format!("{prefixo}{n}"))) { n += 1; }
                let nome = format!("{prefixo}{n}");
                self.folha = self.workbook.criar_aba(nome.clone());
                self.nome_aba = nome;
                self.editando_nome_aba = false;
                if mantendo_formula {
                    self.linha=0;self.coluna=0;self.fim_selecao=(0,0);
                    self.linha_inicio=0;self.coluna_inicio=0;
                    self.endereco=cell_name(0,0);
                } else {self.trocar_selecao(0,0);}
                self.alterado = true; Task::none()
            }
            Msg::DuplicarAba => {
                if self.editando_nome_aba {
                    let tarefa = self.salvar_nome_aba();
                    if self.editando_nome_aba { return tarefa; }
                }
                if !self.confirmar() { return Task::none(); }
                let Some(origem) = self.workbook.aba(self.folha).map(|a| a.id) else { return Task::none(); };
                let Some(nova) = self.workbook.duplicar_aba(self.folha) else { return Task::none(); };
                // O congelamento é da GUI (por id de aba): a cópia herda o da original.
                if let (Some(&c), Some(id)) = (self.congelados.get(&origem), self.workbook.aba(nova).map(|a| a.id)) {
                    self.congelados.insert(id, c);
                }
                self.folha = nova;
                self.nome_aba = self.workbook.nomes().get(nova).cloned().unwrap_or_default();
                self.editando_nome_aba = false;
                let (fixas_linhas,fixas_colunas)=self.congelamento_atual();
                self.linha_inicio=fixas_linhas;
                self.coluna_inicio=fixas_colunas;
                self.trocar_selecao(0,0);
                self.alterado = true;
                self.status = trf!("Aba duplicada: {}", self.nome_aba);
                Task::none()
            }
            Msg::OcultarAba(i) => {
                if !self.confirmar() { return Task::none(); }
                self.contexto = None;
                let nome = self.workbook.aba(i).map(|a| a.sheet.name.clone())
                    .unwrap_or_else(|| tr!("Aba").into());
                match self.workbook.ocultar_aba(i) {
                    Ok(primeira_visivel) => {
                        let tarefa = if self.folha == i {
                            self.update(Msg::Aba(primeira_visivel))
                        } else { Task::none() };
                        self.alterado = true;
                        self.status = trf!("Aba \"{nome}\" ocultada. Reexiba pelo menu de abas.");
                        tarefa
                    }
                    Err(erro) => {
                        self.status = erro.to_owned();
                        Task::none()
                    }
                }
            }
            Msg::MostrarAbasOcultas => {
                if self.workbook.abas_ocultas().is_empty() {
                    self.status = tr!("Não existem abas ocultas.").into();
                } else {
                    self.reexibir_abas_aberto = true;
                    self.contexto = None;
                }
                Task::none()
            }
            Msg::FecharAbasOcultas => {
                self.reexibir_abas_aberto = false;
                Task::none()
            }
            Msg::ReexibirAba(i) => {
                let nome = self.workbook.aba(i).map(|a| a.sheet.name.clone())
                    .unwrap_or_else(|| tr!("Aba").into());
                if self.workbook.reexibir_aba(i) {
                    self.reexibir_abas_aberto = false;
                    self.alterado = true;
                    let tarefa = self.update(Msg::Aba(i));
                    self.status = trf!("Aba \"{nome}\" reexibida.");
                    tarefa
                } else {
                    self.status = tr!("A aba não está oculta ou não existe.").into();
                    Task::none()
                }
            }
            Msg::MoverAba(de, para) => self.mover_aba_para(de, para),
            Msg::CliqueAba(i) => self.clicar_aba(i),
            Msg::CancelarNomeAba => { self.cancelar_renomeacao(); Task::none() }
            Msg::NomearAba(nome) => { self.nome_aba = nome; self.erro_nome_aba = None; Task::none() }
            Msg::EditarNomeAba => self.abrir_renomeacao(),
            Msg::RenomearAba => self.salvar_nome_aba(),
            Msg::RemoverAba => {
                if self.editando_nome_aba { self.cancelar_renomeacao(); }
                let removida=self.workbook.aba(self.folha).map(|a|a.id);
                if self.workbook.remover_aba(self.folha) {
                    if let Some(id)=removida { self.congelados.remove(&id); }
                    self.folha = self.folha.min(self.workbook.len()-1);
                    let (fixas_linhas,fixas_colunas)=self.congelamento_atual();
                    self.linha_inicio=fixas_linhas;
                    self.coluna_inicio=fixas_colunas;
                    self.nome_aba = self.workbook.nomes().get(self.folha).cloned().unwrap_or_default();
                    self.trocar_selecao(0,0); self.alterado=true; self.status=tr!("Aba excluída").into();
                } else { self.status=tr!("A última aba visível não pode ser excluída").into(); }
                Task::none()
            }
            Msg::AlterarEndereco(t) => {
                self.endereco = t;
                self.endereco_digitado = true;
                Task::none()
            }
            Msg::IrPara(t) => {
                self.selecoes_extras.clear();
                self.endereco_digitado = false;
                let entrada = t.trim();
                if entrada.is_empty() {
                    self.endereco = cell_name(self.linha, self.coluna);
                    return Task::none();
                }
                // O TypeScript tenta primeiro nomes existentes. Usar Unicode
                // uppercase, como Workbook::definir_nome, preserva acentos.
                let chave = entrada.to_uppercase();
                if self.workbook.nomeados().iter().any(|n| n.nome.to_uppercase() == chave) {
                    return self.ir_para_nome(entrada);
                }
                let t = entrada.to_ascii_uppercase();
                let celula = |endereco: &str| -> Option<(u32, u32)> {
                    let at = endereco.find(|c: char| c.is_ascii_digit())?;
                    let c = col_index(&endereco[..at])?;
                    let r = endereco[at..].parse::<u32>().ok()?;
                    (r > 0 && r <= MAX_ROWS).then_some((r.saturating_sub(1), c))
                };
                let (inicio, fim) = t.split_once(':').unwrap_or((&t, &t));
                if let (Some((r,c)), Some((rf,cf))) = (celula(inicio), celula(fim)) {
                    if !self.confirmar() { return Task::none(); }
                    self.trocar_selecao(r,c);
                    if (r,c) != (rf,cf) { return self.update(Msg::Estender(rf,cf)); }
                } else if nomes_ui::validar_nome(entrada).is_ok() {
                    // Paridade da caixa de nome TS: ao digitar um nome novo
                    // diretamente, nomeia a seleção atual, não perde a área.
                    if !self.confirmar() { return Task::none(); }
                    let Some(aba) = self.workbook.aba(self.folha) else { return Task::none(); };
                    let nome_aba = aba.sheet.name.clone();
                    let mut area = self.area_selecionada();
                    area.sheet = Some(nome_aba);
                    self.workbook.definir_nome(entrada, area);
                    self.alterado = true;
                    self.endereco = entrada.to_owned();
                    self.status = trf!("Intervalo \"{entrada}\" nomeado");
                } else {
                    self.status = tr!("Endereço ou nome inválido. Use A1:C10 ou um nome sem espaços.").into();
                }
                Task::none()
            }
            Msg::Menu => {
                self.contexto = None;
                self.menu_aberto = !self.menu_aberto;
                if self.menu_aberto {
                    self.menu_pagina = menu_principal::Pagina::Inicio;
                    self.menu_caminho.clear();
                    self.menu_cursor = 0;
                }
                Task::none()
            }
            Msg::MenuPagina(pagina) => {
                self.menu_aberto = true;
                self.menu_pagina = pagina;
                self.menu_caminho.clear();
                self.menu_cursor = 0;
                Task::none()
            }
            Msg::CarrosselModelos(d) => {
                let total = menu_principal::modelos::lista().len();
                self.carrossel_modelos = (self.carrossel_modelos as i32 + d)
                    .clamp(0, total.saturating_sub(1) as i32) as usize;
                Task::none()
            }
            Msg::VerTodosModelos => {
                self.ver_todos_modelos = !self.ver_todos_modelos;
                Task::none()
            }
            Msg::BuscarRecentes(texto) => {
                self.busca_recentes = texto;
                Task::none()
            }
            Msg::MenuAbrirRecente(i) => {
                let Some(arquivo) = self.recentes.arquivos.get(i) else {
                    self.status = tr!("Esse arquivo não está mais no histórico.").into();
                    return Task::none();
                };
                self.recente_destino = Some(PathBuf::from(&arquivo.caminho));
                self.menu_aberto = false;
                self.proteger(Pendente::Recente)
            }
            Msg::MenuFixarRecente(i) => {
                self.recentes.alternar_fixado(i);
                Task::none()
            }
            Msg::MenuEsquecerRecente(i) => {
                self.recentes.esquecer(i);
                Task::none()
            }
            Msg::FecharAplicativo => {
                // Excel: Fechar documento, sem encerrar a janela do aplicativo.
                self.menu_aberto = false;
                self.proteger(Pendente::Novo)
            }
            Msg::MenuGrupo(i) => {
                self.contexto = None;
                if i < menu_nativo::catalogo().menus.len() {
                    self.barra_menu_aberta = true;
                    self.menu_grupo = i;
                    self.menu_caminho.clear();
                    self.menu_cursor = 0;
                }
                Task::none()
            }
            Msg::MenuVoltar => {
                if self.menu_caminho.pop().is_none() {
                    self.barra_menu_aberta = false;
                }
                self.menu_cursor = 0;
                Task::none()
            }
            Msg::MenuMover(delta) => {
                if let Some(itens) = menu_nativo::itens(self.menu_grupo, &self.menu_caminho) {
                    if !itens.is_empty() {
                        let len = itens.len() as i32;
                        for passo in 1..=len {
                            let posicao = (self.menu_cursor as i32 + delta * passo).rem_euclid(len) as usize;
                            if self.item_menu_disponivel(&itens[posicao]) {
                                self.menu_cursor = posicao;
                                break;
                            }
                        }
                    }
                }
                Task::none()
            }
            Msg::MenuEscolher(i) => {
                let item = menu_nativo::itens(self.menu_grupo, &self.menu_caminho)
                    .and_then(|itens| itens.get(i));
                if let Some(item) = item {
                    if !item.filhos.is_empty() {
                        self.menu_caminho.push(i);
                        self.menu_cursor = 0;
                    } else if let Some(msg) = item.acao.as_ref().and_then(|a| self.evento_menu(a)) {
                        self.barra_menu_aberta = false;
                        self.menu_caminho.clear();
                        return self.update(msg);
                    }
                }
                Task::none()
            }
            Msg::AbrirContextoNaGrade(tipo, posicao) => {
                // Mantém o seletor de célula legado e só adapta o retângulo
                // do popup. Não há transformação das células ou fórmulas.
                let tarefa = self.update(Msg::AbrirContexto(tipo, posicao.x, posicao.y));
                if self.contexto.is_some() {
                    self.contexto_tamanho = (posicao.largura, posicao.altura);
                }
                tarefa
            }
            Msg::AbrirContexto(tipo, x, y) => {
                // Editando a célula: grava a edição e abre o menu (antes o
                // botão direito não fazia nada). Fica só se a validação recusar.
                if self.editor_formula.ativo && !self.confirmar() { return Task::none(); }
                self.imagem_contexto = None;
                self.paleta_aba_aberta = false;
                self.contexto_tamanho = if matches!(tipo, Contexto::Aba(_)) {
                    (248.0, cores_aba::altura_menu(
                        self.opcoes_contexto(tipo).len(),
                        self.tamanho_principal.height - 180.0,
                    ))
                } else {
                    (crate::geometria_overlay::LARGURA_CONTEXTUAL,
                     crate::geometria_overlay::ALTURA_CONTEXTUAL)
                };
                // Propaga qualquer Task da seleção (aba, linha, coluna) ao
                // runtime Iced, em vez de descartá-la silenciosamente.
                let tarefa_selecao = match tipo {
                    Contexto::Celula(r, c) => {
                        let area = self.area_selecionada();
                        // A imagem na célula age sobre uma célula só: o clique nela
                        // a ativa mesmo dentro da seleção, ou o menu não a enxerga.
                        let tem_imagem = self.workbook.aba(self.folha).is_some_and(|a|
                            feathersheet_core::funcoes::imagem_base64::eh_imagem_na_celula(&a.sheet.get_raw(r, c)));
                        if r < area.r0 || r > area.r1 || c < area.c0 || c > area.c1 || tem_imagem {
                            self.trocar_selecao(r, c);
                        }
                        Task::none()
                    }
                    Contexto::Linha(r) => {
                        let a = self.area_selecionada();
                        if self.selecao_e_linhas_inteiras() && (a.r0..=a.r1).contains(&r) {
                            // Clique sobre uma faixa já selecionada não a reduz.
                            Task::none()
                        } else { self.update(Msg::SelecionarLinha(r)) }
                    }
                    Contexto::Coluna(c) => {
                        let a = self.area_selecionada();
                        if self.selecao_e_colunas_inteiras() && (a.c0..=a.c1).contains(&c) {
                            Task::none()
                        } else { self.update(Msg::SelecionarColuna(c)) }
                    },
                    Contexto::Aba(i) => self.update(Msg::Aba(i)),
                };
                self.menu_aberto = false;
                self.contexto_sub = None;
                self.contexto = if matches!(tipo, Contexto::Aba(_)) {
                    let xx = cores_aba::x_menu_aba(
                        self.ponteiro_abas_x, self.tamanho_principal.width, 248.0
                    );
                    Some((tipo, xx, 0.0))
                } else { Some((tipo, x, y)) };
                tarefa_selecao
            }
            Msg::PonteiroSobreAbas(x) => {
                if x.is_finite() { self.ponteiro_abas_x = x; }
                Task::none()
            }
            Msg::AlternarPaletaAba => {
                self.paleta_aba_aberta = true;
                self.contexto_tamanho = (
                    248.0, cores_aba::altura_menu(7, self.tamanho_principal.height - 180.0)
                );
                Task::none()
            }
            Msg::VoltarMenuAba => {
                self.paleta_aba_aberta = false;
                self.contexto_tamanho = (
                    248.0, cores_aba::altura_menu(
                        self.opcoes_contexto(Contexto::Aba(self.folha)).len(),
                        self.tamanho_principal.height - 180.0
                    )
                );
                Task::none()
            }
            Msg::PintarAba(cor) => {
                if self.workbook.editar(self.folha, |aba| {
                    aba.tab_color = cor.map(str::to_owned);
                }).is_some() {
                    self.alterado = true;
                    self.status = if cor.is_some() {
                        tr!("Cor da aba atualizada").into()
                    } else { tr!("Cor da aba removida").into() };
                }
                self.contexto = None;
                self.paleta_aba_aberta = false;
                Task::none()
            }
            Msg::ContextoSub(sub) => {
                self.contexto_sub = sub;
                Task::none()
            }
            Msg::ContextoExecutar(acao) => {
                self.contexto = None;
                self.contexto_sub = None;
                self.update(*acao)
            }
            Msg::FecharContexto => {
                self.contexto = None;
                self.contexto_sub = None;
                self.paleta_aba_aberta = false;
                Task::none()
            }
            Msg::ContextoEscolher(indice) => {
                let acao = self.contexto.and_then(|(tipo, _, _)| {
                    self.opcoes_contexto(tipo).into_iter().nth(indice).flatten()
                        .map(|(_, evento)| evento)
                });
                if !matches!(&acao, Some(Msg::AlternarPaletaAba | Msg::ContextoSub(_))) {
                    self.contexto = None;
                    self.contexto_sub = None;
                }
                if let Some(evento) = acao { self.update(evento) } else { Task::none() }
            }
            Msg::FecharMenu => {
                (self.menu_aberto, self.barra_menu_aberta) = (false, false);
                self.menu_caminho.clear(); Task::none()
            }
            Msg::JanelaRedimensionada(id, size) => {
                if self.janelas.principal == Some(id) { self.tamanho_principal = size; }
                // O código das macros usa o tamanho da janela para a largura mínima.
                if self.janelas.tipo(id) == Some(Auxiliar::Macros) { self.macro_visual.codigo.janela = size; }
                Task::none()
            }
            Msg::AlternarGrupoFaixa(indice) => {
                self.faixa_grupo_aberto = if self.faixa_grupo_aberto==Some(indice) {
                    None
                } else {Some(indice)};
                if self.faixa_grupo_aberto.is_some() {
                    self.faixa_extras_aberto=false;
                    self.menu_aberto=false;
                    self.colar_menu_aberto=false;
                    self.orientacao_menu_aberto=false;
                    self.bordas_menu_aberto=false;
                    self.condicional_menu_aberto=false;
                    self.tabelas_menu_aberto=false;
                    self.estilos_menu_aberto=false;
                }
                Task::none()
            }
            Msg::FecharGrupoFaixa => {
                self.faixa_grupo_aberto=None;
                Task::none()
            }
            Msg::AlternarMenuExtras => {
                self.faixa_grupo_aberto=None;
                self.faixa_extras_aberto = !self.faixa_extras_aberto;
                if self.faixa_extras_aberto {
                    self.menu_aberto = false;
                    self.colar_menu_aberto = false;
                    self.bordas_menu_aberto = false;
                    self.condicional_menu_aberto = false;
                    self.contexto = None;
                    self.cor_ativa = None;
                    self.tabela_popup_filtro = false;
                }
                Task::none()
            }
            Msg::FecharMenuExtras => {
                self.faixa_extras_aberto = false;
                Task::none()
            }
            Msg::AbaFaixa(aba) => {
                self.faixa_grupo_aberto=None;
                self.faixa_extras_aberto = false;
                self.menu_aberto = false;
                self.orientacao_menu_aberto = false;
                self.colar_menu_aberto = false;
                self.divisao_personalizada = false;
                self.bordas_menu_aberto = false;
                self.tabelas_menu_aberto = false;
                self.estilos_menu_aberto = false;
                self.edicao_menu = None;
                self.tabela_popup_filtro = false;
                if aba == AbaFaixa::Tabela {
                    // A Ribbon contextual trabalha com a tabela ativa na seleção.
                    self.acao_tabela(tabelas::Acao::Abrir)
                } else {
                    self.selecionar_faixa(aba);
                    Task::none()
                }
            }
            Msg::Configuracoes => { self.menu_aberto = false; self.alternar(Auxiliar::Configuracoes) }
            Msg::Ajuda => {
                self.menu_aberto = false;
                // F1 na fórmula consulta exatamente a chamada interna sob o cursor.
                if let Some((assinatura, _, _, _)) = self.dica_funcao_ativa() {
                    if let Some(nome) = assinatura.split('(').next() {
                        ajuda::abrir_funcao(&mut self.ajuda_estado, nome);
                    }
                }
                self.alternar(Auxiliar::Ajuda)
            }
            Msg::Monitor => { self.menu_aberto = false; self.alternar(Auxiliar::Monitor) }
            Msg::PrimeirosPassosAbrir => { self.menu_aberto = false; self.alternar(Auxiliar::PrimeirosPassos) }
            Msg::PrimeirosPassos(evento) => self.evento_primeiros_passos(evento),
            Msg::NovidadesAbrir => { self.menu_aberto = false; self.novidades_versao = 0; self.alternar(Auxiliar::Novidades) }
            Msg::Novidades(evento) => self.evento_novidades(evento),
            Msg::FaixaSalvamentoFechar => { self.faixa_salvamento = None; Task::none() }
            Msg::AvisoXlsxFechar => { self.aviso_xlsx = None; Task::none() }
            Msg::AvisoMacrosFechar => { self.aviso_macros = false; Task::none() }
            Msg::AtivarMacros => {
                (self.macros_bloqueadas, self.aviso_macros) = (false, false);
                self.status = tr!("Macros ativadas neste arquivo.").into();
                Task::none()
            }
            Msg::ProgressoPulso => { progresso_abertura::animar(); Task::none() },
            Msg::ResumoPulso => {
                self.avancar_resumo();
                Task::none()
            }
            Msg::CapturarTela => {
                // FEATHERSHEET_CAPTURAR_ABRIR=arquivo: da tela inicial, abre como o
                // "Abrir…" depois do diálogo e fotografa no meio da leitura.
                if let Some(arquivo) = std::env::var_os("FEATHERSHEET_CAPTURAR_ABRIR") {
                    if self.passo_captura == 0 {
                        self.passo_captura = 1;
                        return self.update(Msg::ArquivoEscolhido(Some(PathBuf::from(arquivo))));
                    }
                    self.passo_captura = 2;
                    return match self.janelas.principal {
                        Some(id) => Task::done(Msg::TelaCapturada(self.fotografar(id))),
                        None => Task::none(),
                    };
                }
                // Arquivo aberto pela linha de comando: espera terminar de carregar.
                if self.carregando { return Task::none(); }
                match self.passo_captura {
                    0 => {
                        // Planilha em branco; FEATHERSHEET_CAPTURAR_CELULA=XFD1 vai até ela.
                        self.em_tela_inicio = false;
                        // FEATHERSHEET_CAPTURAR_MODELO=queda-tensao[:aba]: abre o modelo (e a aba, base 0).
                        if let Ok(spec) = std::env::var("FEATHERSHEET_CAPTURAR_MODELO") {
                            let (id, aba) = spec.split_once(':').unwrap_or((&spec, "0"));
                            if let Some(m) = menu_principal::modelos::lista().iter().find(|m| m.id == id) {
                                let _ = self.executar_pendente(Pendente::Modelo(m.id));
                                self.folha = aba.parse().unwrap_or(0);
                            }
                        }
                        // FEATHERSHEET_CAPTURAR_ABA=2: mostra essa aba (índice a partir de 0).
                        if let Some(i) = std::env::var("FEATHERSHEET_CAPTURAR_ABA").ok().and_then(|v| v.parse().ok()) {
                            let _ = self.update(Msg::Aba(i));
                        }
                        // FEATHERSHEET_CAPTURAR_MENU_CLASSICO=1: faixa recolhida (sem gravar) e esse menu aberto.
                        if let Some(i) = std::env::var("FEATHERSHEET_CAPTURAR_MENU_CLASSICO").ok().and_then(|v| v.parse().ok()) {
                            self.config.faixa_recolhida = true;
                            let _ = self.update(Msg::MenuGrupo(i));
                        }
                        // FEATHERSHEET_CAPTURAR_CELULAS="A1==SEQUÊNCIA(5)|B1=texto": escreve antes.
                        if let Ok(lista) = std::env::var("FEATHERSHEET_CAPTURAR_CELULAS") {
                            for item in lista.split('|') {
                                let Some((endereco, conteudo)) = item.split_once('=') else { continue };
                                if let Some((r, c)) = feathersheet_core::celulas::refs::parse_cell_ref(endereco) {
                                    self.workbook.escrever_celula(self.folha, r, c, conteudo.to_string());
                                }
                            }
                        }
                        // Aceita intervalo: FEATHERSHEET_CAPTURAR_CELULA=A1:C3.
                        let alvo = std::env::var("FEATHERSHEET_CAPTURAR_CELULA").unwrap_or_default();
                        let (inicio, fim) = alvo.split_once(':').unwrap_or((&alvo, &alvo));
                        if let Some((r, c)) = feathersheet_core::celulas::refs::parse_cell_ref(inicio) {
                            self.trocar_selecao(r, c);
                            // A célula fica à vista mesmo se o arquivo abriu rolado.
                            if !(self.linha_inicio..self.linha_inicio + 15).contains(&r) {
                                self.linha_inicio = r.saturating_sub(3);
                            }
                            if let Some(f) = feathersheet_core::celulas::refs::parse_cell_ref(fim) {
                                self.fim_selecao = f;
                            }
                        }
                        // FEATHERSHEET_CAPTURAR_ZOOM=75 muda o zoom só nesta execução (não grava).
                        if let Some(z) = std::env::var("FEATHERSHEET_CAPTURAR_ZOOM").ok()
                            .and_then(|v| v.parse::<u16>().ok()) {
                            self.config.zoom = z.clamp(self.config.zoom_minimo, self.config.zoom_maximo);
                        }
                        // FEATHERSHEET_CAPTURAR_MACRO="confirm('Gerar?')": roda a macro (pergunta, mensagens).
                        if let Ok(codigo) = std::env::var("FEATHERSHEET_CAPTURAR_MACRO") {
                            let _ = self.rodar_macro(historico_macros::GatilhoMacro::manual(tr!("modo de captura")), tr!("Macro de teste"), &codigo, Vec::new());
                        }
                        // FEATHERSHEET_CAPTURAR_CAIXAS=1: caixas de seleção na seleção da
                        // FEATHERSHEET_CAPTURAR_CELULA, uma sim, uma não marcada.
                        if std::env::var("FEATHERSHEET_CAPTURAR_CAIXAS").is_ok() {
                            let area = self.area_selecionada().normalized();
                            self.inserir_caixas();
                            for r in (area.r0..=area.r1).step_by(2) {
                                self.alternar_caixa(r, area.c0);
                            }
                            self.linha = area.r0;
                            self.coluna = area.c0;
                            self.fim_selecao = (area.r1, area.c1);
                        }
                        // FEATHERSHEET_CAPTURAR_TINTA=1 (ou "menu"): aba Desenhar com traços de exemplo.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_TINTA") {
                            self.preparar_captura_tinta(&modo);
                        }
                        // FEATHERSHEET_CAPTURAR_OBJETOS=1 (ou "menu", "painel"): formas de exemplo, a estrela selecionada.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_OBJETOS") {
                            self.preparar_captura_objetos(&modo);
                        }
                        // FEATHERSHEET_CAPTURAR_IMAGEM=painel|menu: imagem de exemplo selecionada,
                        // com o painel Formatar imagem ou o menu do botão direito aberto.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_IMAGEM") {
                            self.preparar_captura_imagem(&modo);
                        }
                        // FEATHERSHEET_CAPTURAR_CONDICIONAL=escala|barra|estilo (+cor abre o seletor
                        // da primeira cor): diálogo de formatação condicional.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_CONDICIONAL") {
                            let tipo = if modo.starts_with("barra") { TipoCondicional::Barra }
                                else if modo.starts_with("estilo") { TipoCondicional::Maior } else { TipoCondicional::Escala };
                            let _ = self.update(Msg::CondicionalPredefinida(tipo));
                            if modo.ends_with("+cor") {
                                let _ = self.update(Msg::CondicionalCorAbrir(if tipo == TipoCondicional::Escala {
                                    condicional_ui::CorRegra::Min } else { condicional_ui::CorRegra::Principal }));
                            }
                        }
                        // FEATHERSHEET_CAPTURAR_MENU_ABA=1: botão direito na primeira aba.
                        if std::env::var_os("FEATHERSHEET_CAPTURAR_MENU_ABA").is_some() {
                            let _ = self.update(Msg::AbrirContexto(Contexto::Aba(0), 0.0, 0.0));
                        }
                        // FEATHERSHEET_CAPTURAR_BARRA=1 abre "Personalizar barra de status".
                        if std::env::var_os("FEATHERSHEET_CAPTURAR_BARRA").is_some() {
                            self.barra_status_menu = true;
                        }
                        // FEATHERSHEET_CAPTURAR_FAIXA=formulas abre essa aba da faixa.
                        if let Ok(chave) = std::env::var("FEATHERSHEET_CAPTURAR_FAIXA") {
                            self.faixa_ativa = AbaFaixa::de_chave_persistencia(&chave);
                        }
                        // FEATHERSHEET_CAPTURAR_COMENTARIOS="B2=texto|D4!=à mostra|sobre:B2|editor".
                        if let Ok(lista) = std::env::var("FEATHERSHEET_CAPTURAR_COMENTARIOS") {
                            self.preparar_captura_comentarios(&lista);
                        }
                        // FEATHERSHEET_CAPTURAR_RASTREIO="pp d": p = um nível de
                        // precedentes, d = um nível de dependentes, na célula ativa.
                        for passo in std::env::var("FEATHERSHEET_CAPTURAR_RASTREIO").unwrap_or_default().chars() {
                            let _ = match passo {
                                'p' => self.evento_formulas(formulas_ui::Evento::RastrearPrecedentes),
                                'd' => self.evento_formulas(formulas_ui::Evento::RastrearDependentes),
                                _ => Task::none(),
                            };
                        }
                        // FEATHERSHEET_CAPTURAR_TOPICOS=subtotal aplica o Subtotal na lista
                        // da célula ativa; "janela" só abre a janela; "nivel2" recolhe ao nível 2.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_TOPICOS") {
                            let _ = self.evento_topicos(topicos_ui::Evento::SubtotalAbrir);
                            if modo != "janela" {
                                let _ = self.evento_topicos(topicos_ui::Evento::SubtotalAplicar);
                            }
                            if modo == "nivel2" {
                                let _ = self.evento_topicos(topicos_ui::Evento::Nivel(false, 2));
                            }
                        }
                        // FEATHERSHEET_CAPTURAR_LISTA="Sim;Não": validação de lista na célula
                        // ativa, com a lista aberta.
                        if let Ok(lista) = std::env::var("FEATHERSHEET_CAPTURAR_LISTA") {
                            use feathersheet_core::operacoes::validation::{QuandoValidacao, Regra, Validacao};
                            let regra = Validacao {
                                ranges: vec![feathersheet_core::celulas::refs::Rect::cell(self.linha, self.coluna)],
                                regra: Regra::Lista { valores: lista.split(';').map(str::to_owned).collect() },
                                quando: QuandoValidacao::Recusar, mensagem: None, mensagens: Default::default(),
                            };
                            let json = serde_json::to_string(&vec![regra]).unwrap_or_default();
                            self.workbook.editar(self.folha, |s| s.va = Some(json));
                            let _ = self.update(Msg::ValidacaoListaAlternar);
                        }
                        // FEATHERSHEET_CAPTURAR_ABRIR_LISTA=1 abre a lista da validação que já existe na célula.
                        if std::env::var_os("FEATHERSHEET_CAPTURAR_ABRIR_LISTA").is_some() {
                            let _ = self.update(Msg::ValidacaoListaAlternar);
                        }
                        // FEATHERSHEET_CAPTURAR_VALIDACAO=1 abre o diálogo Validação de dados.
                        if std::env::var_os("FEATHERSHEET_CAPTURAR_VALIDACAO").is_some() {
                            let _ = self.update(Msg::ValidacaoAbrir);
                        }
                        // FEATHERSHEET_CAPTURAR_PROTECAO=1 abre a janela Proteger planilha.
                        if std::env::var_os("FEATHERSHEET_CAPTURAR_PROTECAO").is_some() {
                            let _ = self.evento_protecao(protecao_ui::Evento::Abrir);
                        }
                        // FEATHERSHEET_CAPTURAR_INICIO="duplo,grade,menu:bordas": Página Inicial (desenho_borda_ui).
                        if let Ok(spec) = std::env::var("FEATHERSHEET_CAPTURAR_INICIO") {
                            self.preparar_captura_inicio(&spec);
                        }
                        // FEATHERSHEET_CAPTURAR_MENU=Lógica abre o menu dessa categoria.
                        if let Ok(nome) = std::env::var("FEATHERSHEET_CAPTURAR_MENU") {
                            if nome == "Congelar" {
                                self.congelar_popup = (330., 150.);
                                self.congelar_menu_aberto = true;
                            } else if nome == "Criar" {
                                let _ = self.evento_formulas(formulas_ui::Evento::CriarAbrir);
                            } else {
                                self.formulas_menu = formulas_ui::menu_por_nome(&nome);
                            }
                        }
                        // FEATHERSHEET_CAPTURAR_FILTRO=cor|numero|top10|data|periodo|ordcor|lista|menu:
                        // tabela de exemplo com o menu da coluna (e o submenu) aberto.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_FILTRO") {
                            self.preparar_captura_filtro(&modo);
                        }
                        // FEATHERSHEET_CAPTURAR_DIALOGO=nomes|erro|estatisticas|perfil…: abre o diálogo.
                        if let Ok(nome) = std::env::var("FEATHERSHEET_CAPTURAR_DIALOGO") {
                            self.preparar_captura_dialogo(&nome);
                        }
                        self.passo_captura = 1;
                        // FEATHERSHEET_CAPTURAR_GRAFICO=tipo|eixos|pontos|cor|…: gráfico de exemplo
                        // estilizado com o painel Formatar gráfico aberto (e rolado) nesse grupo.
                        if let Ok(modo) = std::env::var("FEATHERSHEET_CAPTURAR_GRAFICO") {
                            return self.preparar_captura_grafico(&modo);
                        }
                        // FEATHERSHEET_CAPTURAR_AJUDA=SOMA abre a Ajuda F1 buscando essa função
                        // (ou macro) e fotografa a janela dela; `SOMA+copiado` simula também o
                        // clique em Copiar no primeiro exemplo (mostra o retorno "Copiado").
                        if let Ok(pedido) = std::env::var("FEATHERSHEET_CAPTURAR_AJUDA") {
                            let (busca, copiado) = pedido.split_once("+copiado").map_or((pedido.as_str(), false), |(b, _)| (b, true));
                            ajuda::abrir_funcao(&mut self.ajuda_estado, busca);
                            ajuda::abrir_pagina_da_busca(&mut self.ajuda_estado);
                                                        if copiado {
                                if let Some(texto) = ajuda::primeiro_exemplo(&self.ajuda_estado) {
                                    self.ajuda_estado.copiado = Some(texto);
                                }
                            }
                            return self.alternar(Auxiliar::Ajuda);
                        }
                        // FEATHERSHEET_CAPTURAR_JANELA=inspecao: adiciona a seleção à
                        // Janela de inspeção e fotografa ela em vez da principal.
                        if std::env::var("FEATHERSHEET_CAPTURAR_JANELA").as_deref() == Ok("inspecao") {
                            return self.evento_inspecao(inspecao_ui::Evento::Adicionar);
                        }
                        // FEATHERSHEET_CAPTURAR_JANELA=condicional: uma regra de exemplo e a janela aberta.
                        if std::env::var("FEATHERSHEET_CAPTURAR_JANELA").as_deref() == Ok("condicional") {
                            let _ = self.update(Msg::CondicionalPredefinida(TipoCondicional::Maior));
                            let _ = self.update(Msg::CondicionalIntervalo("B2:B20".into()));
                            let _ = self.update(Msg::CondicionalAdicionar);
                            return self.update(Msg::CondicionalNovaRegra);
                        }
                        // FEATHERSHEET_CAPTURAR_JANELA=historico_macros: roda macros de exemplo
                        // e fotografa o editor (Alt+F11) com o Histórico aberto.
                        if std::env::var("FEATHERSHEET_CAPTURAR_JANELA").as_deref() == Ok("historico_macros") {
                            return self.preparar_captura_historico_macros();
                        }
                        // FEATHERSHEET_CAPTURAR_JANELA=busca_macros: editor com módulo de
                        // exemplo, busca do Explorador, Ctrl+H aberto e a calha de números.
                        if std::env::var("FEATHERSHEET_CAPTURAR_JANELA").as_deref() == Ok("busca_macros") {
                            return self.preparar_captura_busca_macros();
                        }
                        // FEATHERSHEET_CAPTURAR_JANELA=primeiros:<passo 1–5>, novidades[:<versão 1–3>],
                        // monitor (F12), config:<seção 1–7> ou formatar:<aba 1–6> (Formatar células).
                        if let Ok(janela) = std::env::var("FEATHERSHEET_CAPTURAR_JANELA") {
                            let (nome, numero) = janela.split_once(':').map_or((janela.as_str(), 1), |(n, i)| (n, i.parse().unwrap_or(1)));
                            match nome {
                                // Faixa "sem salvamento automático" (1 = .parquet, 2 = pasta grande).
                                "faixa_salvamento" => {
                                    self.origem = Some(PathBuf::from("dados.parquet"));
                                    self.aviso_salvamento_de = Some(self.nome_documento());
                                    self.faixa_salvamento = Some(if numero == 2 {
                                        (tr!("Esta pasta tem 633 milhões de células: grande demais para cópia automática. Nem a recuperação nem o salvamento automático a gravam; salve com Ctrl+S.").into(), true)
                                    } else {
                                        (tr!("O salvamento automático não vale para arquivos .parquet. Até salvar como .fshz, só a recuperação automática protege esta planilha.").into(), false)
                                    });
                                    return Task::none();
                                }
                                "primeiros" => {
                                    self.primeiros_passos_passo = numero.max(1) - 1;
                                    return self.alternar(Auxiliar::PrimeirosPassos);
                                }
                                "novidades" => {
                                    self.novidades_versao = numero.max(1) - 1;
                                    return self.alternar(Auxiliar::Novidades);
                                }
                                // Módulos em pastas e o código de evento da Planilha1 aberto.
                                "pastas_macros" => {
                                    let m = |n: &str, c: &str| feather_macros::Macro { name: n.into(), code: c.into(), auto: false };
                                    self.macro_catalogo = feather_macros::Catalogo { macros: vec![
                                        m("macros_auxiliares/macro_1", "log(1);"),
                                        m("macros_auxiliares/macro_2", "log(2);"),
                                        m("macro_para_preencher planilha/macro_3", "log(3);"),
                                        m("Relatorio", "log('r');"),
                                        m("Eventos/Planilha1/aoMudar", "// Destaca o que mudou\nlog(valores.intervalo);\n"),
                                    ] };
                                    self.sincronizar_eventos();
                                    self.evento_macro_visual(macros::EventoVisual::Documento("planilha:Planilha1".into()));
                                    return self.alternar(Auxiliar::Macros);
                                }
                                "monitor" => return self.alternar(Auxiliar::Monitor),
                                // config:<seção 1–7> (6 = IA (MCP), 7 = Sobre).
                                "config" => {
                                    self.config_secao = (numero.max(1) - 1).min(configuracoes_ui::SECAO_SOBRE);
                                    return self.alternar(Auxiliar::Configuracoes);
                                }
                                "formatar" => {
                                    use super::formatar_celulas::Aba;
                                    let aba = [Aba::Numero, Aba::Alinhamento, Aba::Fonte, Aba::Borda,
                                        Aba::Preenchimento, Aba::Protecao][numero.clamp(1, 6) - 1];
                                    return self.abrir_formatar_celulas(aba);
                                }
                                _ => {}
                            }
                        }
                    }
                    1 => {
                        // Clique só depois do primeiro desenho da grade: o
                        // tamanho real do Canvas já está guardado.
                        // FEATHERSHEET_CAPTURAR_CLIQUE=ultima clica na última célula inteira
                        // à vista (ou num endereço, B30); "ultima+seta" ainda aperta ↓ e →.
                        if let Some(alvo) = std::env::var("FEATHERSHEET_CAPTURAR_CLIQUE").ok()
                            .filter(|_| !self.captura_clicou) {
                            self.captura_clicou = true;
                            let (alvo, seta) = alvo.split_once('+').map_or((alvo.as_str(), false), |(a, _)| (a, true));
                            let celula = if alvo == "ultima" {
                                let (l, c) = self.pagina_da_grade();
                                let (lf, cf) = self.congelamento_atual();
                                Some((self.linha_inicio.max(lf) + l - 1, self.coluna_inicio.max(cf) + c - 1))
                            } else {
                                feathersheet_core::celulas::refs::parse_cell_ref(alvo)
                            };
                            if let Some((r, c)) = celula {
                                let _ = self.update(Msg::Selecionar(r, c));
                                if seta {
                                    let _ = self.update(Msg::Mover(1, 0));
                                    let _ = self.update(Msg::Mover(0, 1));
                                }
                                self.status = trf!("Captura: zoom {}%, célula {}, origem {}",
                                    self.config.zoom, cell_name(self.linha, self.coluna),
                                    cell_name(self.linha_inicio, self.coluna_inicio));
                            }
                            // Fotografa no próximo passo, com a grade já redesenhada.
                            return Task::none();
                        }
                        self.passo_captura = 2;
                        let auxiliar = if std::env::var_os("FEATHERSHEET_CAPTURAR_AJUDA").is_some() {
                            self.janelas.id(Auxiliar::Ajuda)
                        } else {
                            match std::env::var("FEATHERSHEET_CAPTURAR_JANELA").as_deref() {
                                Ok("inspecao") => self.janelas.id(Auxiliar::Inspecao),
                                Ok("condicional") => self.janelas.id(Auxiliar::Condicional),
                                Ok("historico_macros" | "busca_macros") => self.janelas.id(Auxiliar::Macros),
                                Ok(j) if j.starts_with("primeiros") => self.janelas.id(Auxiliar::PrimeirosPassos),
                                Ok(j) if j.starts_with("novidades") => self.janelas.id(Auxiliar::Novidades),
                                Ok("monitor") => self.janelas.id(Auxiliar::Monitor),
                                Ok(j) if j.starts_with("config") => self.janelas.id(Auxiliar::Configuracoes),
                                Ok("pastas_macros") => self.janelas.id(Auxiliar::Macros),
                                Ok(j) if j.starts_with("formatar") => self.janelas.id(Auxiliar::FormatarCelulas),
                                _ => None,
                            }
                        };
                        if let Some(id) = auxiliar.or(self.janelas.principal) {
                            return Task::done(Msg::TelaCapturada(self.fotografar(id)));
                        }
                    }
                    _ => {}
                }
                Task::none()
            }
            Msg::TelaCapturada(s) => {
                if let Some(caminho) = std::env::var_os("FEATHERSHEET_CAPTURAR") {
                    let _ = std::fs::write(&caminho, &*s.png);
                }
                Task::window(pelicano::WindowCmd::Exit)
            }
            Msg::CopiaPulso => Task::none(),
            Msg::MonitorPulso => {
                self.recolher_diario_motor();
                // Parado e com o visível calculado: arruma a memória do motor
                // aos poucos, em lotes, até ~30 ms por pulso (500 mil de uma
                // vez davam um tranco por segundo depois de colar 13 milhões
                // de fórmulas).
                if !self.carregando && !self.calculo_pendente {
                    self.workbook.soltar_faixas();
                    let inicio = std::time::Instant::now();
                    let mut movidos = 0;
                    while inicio.elapsed() < Duration::from_millis(30) {
                        let lote = self.workbook.consolidar_calculos(50_000);
                        movidos += lote;
                        if lote < 50_000 { break; }
                    }
                    if movidos > 0 {
                        self.monitor.registrar(feather_ui::monitor::Nivel::Info, tr!("Motor"),
                            &trf!("[motor] {movidos} resultados de fórmula guardados nas colunas"));
                    }
                }
                // Garante ativação mesmo quando a primeira seleção ocorreu
                // antes da assinatura dinâmica dos pulsos de estatísticas.
                self.avancar_resumo();
                if self.aviso_abas.as_ref().is_some_and(|(_, desde)| {
                    desde.elapsed() >= Duration::from_secs(5)
                }) {
                    self.aviso_abas = None;
                }
                self.monitor.amostrar();
                if !self.carregando { self.atualizar_dinamicas_sozinhas(); }
                let recuperar = self.pulso_recuperacao();
                self.avisar_fora_do_salvamento_automatico();
                // Apenas documentos FSHZ existentes, após inatividade e sem editor ativo.
                // Nunca escolhe um caminho ou grava uma importação XLSX sobre o original.
                // Nunca junto com a cópia de recuperação: seriam duas fotografias
                // da pasta na memória e duas gravações no disco ao mesmo tempo.
                if self.config.auto_salvar && self.alterado && !self.carregando && !self.recuperacao.gravando
                    // Não gravar um FSHZ parcial enquanto a conversão está
                    // dentro de uma transação ainda não publicada.
                    && !self.assistente_divisao.as_ref()
                        .is_some_and(|e| e.conversao_em_andamento())
                    && !self.editor_formula.ativo && self.pendente.is_none()
                    && self.caminho.as_ref().is_some_and(|p|
                        p.extension().and_then(|ext| ext.to_str()) == Some("fshz"))
                    && self.auto_salvar_desde.is_some_and(|t| {
                        // O backup-policy.ts do TS controlava custo/volume
                        // do snapshot rápido. No Rust, adaptamos a cadência
                        // sem desativar a gravação FSHZ de documentos grandes.
                        // Conta células ocupadas, sem materializar a planilha.
                        let celulas = (0..self.workbook.len())
                            .filter_map(|i| self.workbook.aba(i))
                            .map(|aba| aba.sheet.len() as u64)
                            .fold(0u64, u64::saturating_add);
                        let intervalo = feathersheet_io::backup_policy::intervalo_autosave(
                            u64::from(self.config.intervalo_auto_salvar) * 1_000,
                            self.auto_salvar_ultimo_custo_ms, celulas, false,
                        );
                        !feathersheet_io::backup_policy::grande_demais_para_copia_automatica(celulas)
                            && t.elapsed() >= Duration::from_millis(intervalo)
                    })
                {
                    // Mesmo em falha de disco, aguarda novo intervalo antes de tentar outra vez.
                    self.auto_salvar_desde = Some(Instant::now());
                    return Task::batch([recuperar, self.update(Msg::Salvar)]);
                }
                recuperar
            }
            Msg::MonitorEvento(evento) => self.monitor.atualizar(evento),
            Msg::AjudaAbrirSecao(secao) => {
                self.ajuda_estado.abrir(ajuda::modelo(), secao, None);
                // Já aberta: só traz para a frente (alternar a fecharia).
                match self.janelas.id(Auxiliar::Ajuda) {
                    Some(id) => Task::window_of(id, pelicano::WindowCmd::Focus),
                    None => self.alternar(Auxiliar::Ajuda),
                }
            }
            Msg::AjudaSecao(secao) => {
                self.ajuda_estado.atualizar(
                    feather_ui::ajuda::Evento::IrSecao(secao), ajuda::modelo()
                );
                Task::none()
            }
            Msg::AjudaEvento(e) => {
                // Copiar: texto exato do bloco vai para a área de transferência e o
                // botão mostra "Copiado" por um instante (sem janela nova).
                let copiar = ajuda::texto_a_copiar(&e).map(str::to_owned);
                self.ajuda_estado.atualizar(e, ajuda::modelo());
                match copiar {
                    Some(texto) => Task::batch([
                        Task::write_clipboard(texto),
                        Task::perform(
                            tokio::time::sleep(Duration::from_millis(1500)),
                            |_| Msg::AjudaEvento(feather_ui::ajuda::Evento::LimparCopiado),
                        ),
                    ]),
                    None => Task::none(),
                }
            }
            Msg::AuxAberta => Task::none(),
            Msg::AuxFechada(id) => {
                if self.janelas.principal == Some(id) { mcp_vivo::encerrar(); vinculos::encerrar(); Task::window(pelicano::WindowCmd::Exit) }
                else {
                    if self.janelas.tipo(id) == Some(Auxiliar::FormatarCelulas) { self.formato_celula = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::DividirTexto) {
                        self.cancelar_trabalho_divisao();
                        self.assistente_divisao = None;
                    }
                    if self.janelas.tipo(id) == Some(Auxiliar::RemoverDuplicados) { self.duplicados = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::OpenQuery) { self.openquery = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::Condicional) { self.condicional_aberto = false; self.condicional_pegando = false; }
                    self.janelas.retirar(id); Task::none()
                }
            }
            Msg::Fechar(id) => {
                if self.janelas.principal == Some(id) { self.proteger(Pendente::Sair) }
                else {
                    if self.janelas.tipo(id) == Some(Auxiliar::FormatarCelulas) { self.formato_celula = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::DividirTexto) {
                        self.cancelar_trabalho_divisao();
                        self.assistente_divisao = None;
                    }
                    if self.janelas.tipo(id) == Some(Auxiliar::RemoverDuplicados) { self.duplicados = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::OpenQuery) { self.openquery = None; }
                    if self.janelas.tipo(id) == Some(Auxiliar::Condicional) { self.condicional_aberto = false; self.condicional_pegando = false; }
                    self.janelas.retirar(id); Task::window_of(id, pelicano::WindowCmd::Close)
                }
            }
            Msg::FecharAux(t) => {
                if t==Auxiliar::DividirTexto {
                    self.cancelar_trabalho_divisao();
                    self.assistente_divisao=None;
                }
                self.janelas.fechar_tipo(t)
            },
            Msg::Minimizar(id) => Task::window_of(id, pelicano::WindowCmd::Minimize),
            Msg::Maximizar(id) => Task::window_of(id, pelicano::WindowCmd::ToggleMaximize),
            Msg::Arrastar(id) => feather_ui::janelas::arrastar_ou_maximizar(id, &mut self.ultimo_clique_barra),
            Msg::MacroTeclado(id,key,mods) => {
                if self.janelas.tipo(id) == Some(Auxiliar::Macros) {
                    return self.atalho_macro(&key, mods).unwrap_or_else(Task::none);
                }
                Task::none()
            }
            Msg::TecladoNoCampo(id,key,mods) => {
                if self.editando_nome_aba { return self.update(Msg::Teclado(id,key,mods)); }
                if self.editor_formula.ativo && self.edicao.starts_with('=') {
                    self.update(Msg::Teclado(id,key,mods))
                } else {Task::none()}
            }
            Msg::Teclado(id, key, mods) => self.processar_teclado(id, key, mods),
            Msg::Tema(b) => {
                self.config.tema_auto = false;
                self.config.escuro = b;
                definir_tema(b, self.config.acento);
                self.config.gravar(); Task::none()
            }
            Msg::TemaAuto => {
                self.config.tema_auto = true;
                definir_tema(escuro_resolvido(true, self.config.escuro), self.config.acento);
                self.config.gravar(); Task::none()
            }
            Msg::Acento(c) => {
                self.config.acento = c.min(ACENTOS.len()-1);
                self.config.marca_personalizada = Some(true);
                definir_tema(escuro_resolvido(self.config.tema_auto, self.config.escuro), self.config.acento);
                feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherSheet,true);
                self.config.gravar(); Task::none()
            }
            Msg::LogoPadrao => {
                self.config.marca_personalizada = Some(false);
                feather_ui::marca::definir_marca(feather_ui::marca::Produto::FeatherSheet,false);
                self.config.gravar(); Task::none()
            }
            Msg::BarraPropria(ativar) => {
                self.config.barra_propria = ativar;
                self.config.gravar();
                Task::none()
            }
            Msg::Faixa => { self.config.faixa_recolhida = !self.config.faixa_recolhida;
                self.barra_menu_aberta = false;
                self.config.gravar(); Task::none() }
            Msg::SecaoConfig(c) => {
                self.config_secao = c.min(configuracoes_ui::SECAO_SOBRE);
                self.atalho_captura = None;
                Task::none()
            }
            Msg::ZoomMinimo(percentual) => {
                self.config.definir_limite_zoom(true, percentual);
                self.config.gravar();
                Task::none()
            }
            Msg::ZoomMaximo(percentual) => {
                self.config.definir_limite_zoom(false, percentual);
                self.config.gravar();
                Task::none()
            }
            Msg::FonteGrade(fonte) => {
                if matches!(fonte, "sans" | "serif" | "mono") {
                    self.config.fonte_grade = fonte.to_owned();
                    self.config.gravar();
                }
                Task::none()
            }
            Msg::CorpoGrade(tamanho) => {
                if [11, 12, 13, 14, 16].contains(&tamanho) {
                    self.config.tamanho_fonte_grade = tamanho;
                    self.config.gravar();
                }
                Task::none()
            }
            Msg::CsvDetectarSeparador(ativo) => {
                self.config.csv_detectar_separador = ativo;
                self.config.gravar();
                Task::none()
            }
            Msg::Idioma(codigo) => {
                if matches!(codigo, "pt-BR" | "en-US" | "es-ES") {
                    self.confirmar();
                    self.config.idioma = codigo.to_owned();
                    feathersheet_core::set_locale_config(self.regionalizacao());
                    ajuda::definir_idioma(codigo);
                    self.config.gravar();
                    self.trocar_selecao(self.linha, self.coluna);
                    self.status = trf!("Regionalização: {codigo}; separador de fórmulas e texto: {}", self.regionalizacao().formula_arg_separator);
                }
                Task::none()
            }
            // O cálculo em si roda no fim de todo `update` (calculo_visivel.rs).
            Msg::CalcularPulso => Task::none(),
            Msg::Nucleos(n) => {
                feathersheet_core::paralelismo::definir(n as usize);
                // Guarda 0 quando é o padrão: num computador novo vale a
                // metade dele, não o número escolhido neste.
                let usado = feathersheet_core::paralelismo::nucleos();
                self.config.nucleos = if usado == feathersheet_core::paralelismo::padrao() { 0 } else { usado as u16 };
                self.config.gravar();
                self.status = trf!("Abrir e calcular: {usado} núcleos");
                Task::none()
            }
            Msg::LimiteMemoriaGb(gb) => {
                self.config.limite_memoria_gb = limite_memoria::normalizar_limite(gb as f64);
                feathersheet_io::memoria::definir_limite_gb(self.config.limite_memoria_gb);
                self.config.gravar();
                Task::none()
            }
            Msg::AutoSalvar(ativo) => {
                self.config.auto_salvar = ativo;
                self.auto_salvar_desde = if ativo && self.alterado { Some(Instant::now()) } else { None };
                self.config.gravar();
                Task::none()
            }
            Msg::IntervaloAutoSalvar(segundos) => {
                if [2, 5, 15, 30, 60].contains(&segundos) {
                    self.config.intervalo_auto_salvar = segundos;
                    self.auto_salvar_desde = if self.alterado { Some(Instant::now()) } else { None };
                    self.config.gravar();
                }
                Task::none()
            }
            Msg::CapturarAtalho(id) => {
                if configuracoes_ui::ATALHOS.iter().any(|(atual, _, _)| *atual == id) {
                    self.atalho_captura = Some(id);
                }
                Task::none()
            }
            Msg::RestaurarAtalhos => {
                self.config.atalhos.clear();
                self.atalho_captura = None;
                self.config.gravar();
                Task::none()
            }
            Msg::Cancelar => {
                // Esc na edição dentro da célula deve cancelar o rascunho
                // imediatamente, inclusive em tabelas com aba contextual ativa.
                if self.edicao_inline.is_some() && self.editor_formula.ativo {
                    let origem = self.editor_formula.origem;
                    self.edicao_inline = None;
                    self.editor_formula.encerrar();
                    if let Some((aba, r, c)) = origem {
                        self.folha = aba;
                        self.nome_aba = self.workbook.nomes().get(aba).cloned().unwrap_or_default();
                        self.trocar_selecao(r, c);
                    }
                    return crate::campo_fx::desfocar();
                }
                if self.pendente.take().is_some() {
                    self.recente_destino = None;
                    self.status = tr!("Operação cancelada; alterações preservadas.").into();
                    return Task::none();
                }
                if self.edicao_menu.take().is_some() { return Task::none(); }
                if self.tabela_popup_totais {
                    self.tabela_popup_totais = false;
                    return Task::none();
                }
                if self.barra_status_menu {
                    self.barra_status_menu = false;
                    return Task::none();
                }
                if self.tabela_popup_filtro {
                    // Esc fecha somente o flyout; outro Esc fecha o popup principal.
                    if self.tabela_filtro_pagina != 0 {
                        return self.acao_tabela(tabelas::Acao::FecharCondicoes);
                    }
                    self.tabela_popup_filtro = false;
                    return Task::none();
                }
                // A Ribbon de tabela não é um modal: Esc só sai da aba
                // contextual, sem descartar silenciosamente o ID da tabela.
                if self.faixa_ativa == AbaFaixa::Tabela {
                    self.faixa_ativa = AbaFaixa::Inicio;
                    return Task::none();
                }
                if self.tabela_area.take().is_some() { return Task::none(); }
                if self.botao_macro_modal.take().is_some() { return Task::none(); }
                if self.arraste_botao_macro.take().is_some() {
                    self.status = tr!("Movimento do botão cancelado").into();
                    return Task::none();
                }
                if self.botao_macro_selecionado.take().is_some() {
                    return Task::none();
                }
                if self.arraste_imagem.take().is_some() {
                    self.status = tr!("Movimento da imagem cancelado").into();
                    return Task::none();
                }
                if self.imagem_selecionada.take().is_some() {
                    return Task::none();
                }
                if self.pincel_formatacao.take().is_some() {
                    self.status = tr!("Pincel de formatação cancelado").into();
                    return Task::none();
                }
                if self.editor_formula.ativo {
                    let origem=self.editor_formula.origem;
                    self.editor_formula.encerrar();
                    self.multilinha = None;
                    self.edicao_inline = None;
                    if let Some((aba,r,c))=origem {
                        self.folha=aba;
                        self.nome_aba=self.workbook.nomes().get(aba).cloned().unwrap_or_default();
                        self.trocar_selecao(r,c);
                    } else {
                        self.edicao=self.workbook.aba(self.folha)
                            .map(|a|feathersheet_core::celulas::locale::apresentar_formula(&a.sheet.get_raw(self.linha,self.coluna), &self.regionalizacao())).unwrap_or_default();
                    }
                    return crate::campo_fx::desfocar();
                }
                if self.busca.aberta { self.busca.aberta = false; return Task::none(); }
                if self.contexto.take().is_some() { return Task::none(); }
                self.pendente = None; self.menu_aberto = false;
                self.recente_destino = None;
                self.editando_nome_aba = false;
                self.edicao = self.workbook.aba(self.folha).map(|a| feathersheet_core::celulas::locale::apresentar_formula(&a.sheet.get_raw(self.linha,self.coluna), &self.regionalizacao())).unwrap_or_default();
                Task::none()
            }
            Msg::Descartar => if let Some(tipo) = self.pendente.take() {
                // Se o seletor Abrir for cancelado (ou um recente falhar),
                // a planilha original permanece alterada e protegida.
                self.executar_pendente(tipo)
            } else { Task::none() },
            Msg::SalvarAntes => self.update(Msg::Salvar),
            Msg::GuardarTemporariamente => self.guardar_temporariamente(),
            _ => unreachable!("mensagem roteada ao dispatcher secundário indevidamente"),
        }
    }

    /// A foto da janela `id` (o `window::screenshot` do Iced): o pelicano desenha a mesma
    /// árvore direto num PNG, no tamanho da janela.
    pub(super) fn fotografar(&self, id: window::Id) -> Foto {
        let tamanho = if self.janelas.principal == Some(id) {
            self.tamanho_principal
        } else {
            // shortcut: a janela redimensionada sai no tamanho de abertura; melhorar guardando o
            // tamanho de cada janela no `JanelaRedimensionada`.
            match self.janelas.tipo(id) {
                Some(Auxiliar::Macros) => self.macro_visual.codigo.janela,
                Some(tipo) => Self::tamanho_aux(tipo),
                None => Size::new(1280., 800.),
            }
        };
        let png = pelicano::capture(&self.view(id), tamanho, 1.0, &self.tema(id), &mut pelicano::Renderer::new(), None);
        Foto { png: Arc::new(png), largura: tamanho.width as u32, altura: tamanho.height as u32 }
    }
}
