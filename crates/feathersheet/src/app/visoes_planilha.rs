//! Renderização da grade, abas e operações visuais de formatação condicional.
use super::*;

impl App {
    pub(super) fn aba_faixa(&self, titulo: &'static str, aba: AbaFaixa) -> Element<'_, Msg> {
        // Contrato visual e callbacks comuns; enum e comandos permanecem no Sheet.
        feather_ui::ribbon::aba_com_padding(
            &feather_ui::ribbon::AbaRibbon::nova(aba, titulo),
            &self.faixa_ativa,
            Msg::AbaFaixa,
            12.0,
        )
    }

    pub(super) fn icone_menu(nome: &str) -> &'static str {
        // O rótulo chega traduzido; o ícone é escolhido pelo texto em português.
        match crate::idioma::original(nome) {
            "Nova aba" | "+ Linha" | "+ Coluna" | "Nova" => "file-plus",
            "Abrir" => "folder-open", "Salvar" | "Salvar como" => "device-floppy",
            "Desfazer" => "arrow-back-up", "Refazer" => "arrow-forward-up",
            "Em cima" | "No meio" | "Embaixo" => "align-center",
            "Tachado" => "strikethrough", "S" => "underline",
            "Recortar" => "cut", "Copiar" => "copy", "Colar" | "Colar especial" => "clipboard",
            "Inserir" => "plus", "Excluir…" => "trash",
            "Excluir linha" | "Excluir coluna" => "trash",
            "Inserir linha acima" | "Inserir linha abaixo" |
            "Inserir coluna à esquerda" | "Inserir coluna à direita" => "table",
            "Limpar conteúdo" => "trash", "Localizar e substituir" => "search",
            "Inserir comentário" => "message-circle", "Editar comentário" => "edit",
            "Excluir comentário" => "trash", "Mostrar comentário" => "eye", "Ocultar comentário" => "eye-off",
            "Renomear aba" => "edit", "Cor da aba…" => "palette",
            "Ocultar aba" => "eye-off", "Reexibir abas…" => "eye",
            "Mover aba à esquerda" | "Mover para o início" => "chevron-left",
            "Mover aba à direita" | "Mover para o fim" => "chevron-right",
            "Ocultar linhas" | "Ocultar colunas" => "eye-off",
            "Reexibir linhas" | "Reexibir colunas" => "eye",
            "AutoSoma" | "Funções..." => "list-numbers",
            "Localizar" => "search", "Limpar valores" => "trash",
            "Formatar..." => "palette", "75%" | "100%" | "125%" => "zoom-in",
            "Tema claro" | "Tema escuro" => "palette",
            "− Linha" | "− Coluna" | "Excluir aba" => "x",
            "R$ Moeda" => "list-numbers", "%" => "list-numbers",
            "N" => "bold", "I" => "italic", "Esquerda" => "align-left",
            "Centro" => "align-center", "Direita" => "align-right",
            "Ajuda · F1" | "Ajuda" | "Referência da API" => "help", "Monitor · F12" | "Monitor" | "Monitor de execução" => "device-desktop",
            "Configurações" => "settings", "Mostrar linhas" => "eye",
            "Ir para A1" => "search", "Abrir .fshz / CSV" => "folder-open",
            "Salvar .fshz" => "device-floppy",
            "Exportar Excel" => "file-export", "A → Z" | "Z → A" => "list",
            "Remover repetidas" => "copy", "Aparar espaços" => "clear",
            "Localizar / substituir" => "search", "∑ AutoSoma" => "list-numbers",
            "Macros · Alt+F11" | "Editor de macros" | "Executar" => "file-plus",
            "Abrir .js" => "folder-open", "Salvar .js" => "device-floppy",
            "Fechar painel" => "x", _ => "table",
        }
    }

    /// Constrói a mesma regra JSON do condicional original em TypeScript.
    /// Ao salvar na Sheet::r, o FSHZ mantém as regras sem gravar estilo por célula.
    /// Carrega uma regra existente para edição; a operação "Salvar" mantém
    /// ID, intervalos originais e prioridade, trocando só a condição.
    pub(super) fn editar_condicional(&mut self, indice: usize) {
        use feathersheet_core::operacoes::rules::Condition;
        let regras = condicional::regras(&self.workbook, self.folha);
        let Some(regra) = regras.get(indice) else { return };
        self.condicional_editando = Some(indice);
        match &regra.condition {
            Condition::Cell(c) => {
                self.condicional_operador = TipoCondicional::de_operador(c.op);
                self.condicional_valor = c.values.first().cloned().unwrap_or_default();
                self.condicional_valor2 = c.values.get(1).cloned().unwrap_or_default();
                self.condicional_cor = c.style.bg.clone().unwrap_or_else(||"#fce8b2".into());
                self.condicional_cor_texto = c.style.color.clone().unwrap_or_default();
                self.condicional_negrito = c.style.bold.unwrap_or(false);
                self.condicional_italico = c.style.italic.unwrap_or(false);
                self.condicional_sublinhado = c.style.underline.unwrap_or(false);
                self.condicional_riscado = c.style.strike.unwrap_or(false);
            }
            Condition::ColorScale(c) => {
                self.condicional_operador = TipoCondicional::Escala;
                self.condicional_cor_min = c.min.color.clone();
                self.condicional_cor = c.max.color.clone();
                self.condicional_ponto_meio = c.mid.is_some();
                if let Some(meio) = &c.mid {
                    self.condicional_cor_meio = meio.color.clone();
                }
            }
            Condition::DataBar(c) => {
                self.condicional_operador = TipoCondicional::Barra;
                self.condicional_cor = c.color.clone();
                self.condicional_cor_negativa = c.negative_color.clone().unwrap_or_default();
                self.condicional_ocultar_valor = c.hide_value;
            }
        }
        self.condicional_aberto = true;
        self.condicional_pegando = false;
        self.condicional_intervalo = regra.ranges.iter()
            .map(|a| endereco_area(&Rect::new(a.r0, a.c0, a.r1, a.c1))).collect::<Vec<_>>().join("; ");
        self.status = trf!("Editando regra {} de {}", indice + 1, regras.len());
    }

    pub(super) fn adicionar_condicional(&mut self, cores_alternadas: bool) {
        use feathersheet_core::operacoes::rules::{
            CellCondition,CellOp,CondStyle,Condition,ConditionalRule,ColorScaleCondition,
            DataBarCondition,Faixa,ScalePoint,ScalePointType,
        };
        self.confirmar();
        use feathersheet_core::operacoes::rules::parse_color;
        if !cores_alternadas {
            let mut cores=vec![self.condicional_cor.as_str()];
            match self.condicional_operador {
                TipoCondicional::Escala => {
                    cores.push(&self.condicional_cor_min);
                    if self.condicional_ponto_meio {cores.push(&self.condicional_cor_meio);}
                }
                TipoCondicional::Barra => {
                    if !self.condicional_cor_negativa.is_empty() {
                        cores.push(&self.condicional_cor_negativa);
                    }
                }
                _ => {
                    if !self.condicional_cor_texto.is_empty() {
                        cores.push(&self.condicional_cor_texto);
                    }
                }
            }
            if cores.iter().any(|cor|parse_color(cor).is_none()) {
                self.status = tr!("Cor condicional inválida: use #RGB ou #RRGGBB").into();
                return;
            }
        }
        let mut lista = condicional::regras(&self.workbook, self.folha);
        let Some(aba) = self.workbook.aba(self.folha) else {return};
        let area = self.area_selecionada();
        let selecao = Faixa {r0:area.r0,c0:area.c0,r1:area.r1,c1:area.c1};
        // O atalho Cores alternadas sempre cria uma NOVA regra,
        // mesmo quando o painel está editando uma anterior.
        let editando = if cores_alternadas { None }
            else { self.condicional_editando.filter(|&i| i < lista.len()) };
        let id = editando.map(|i| lista[i].id.clone())
            .unwrap_or_else(|| format!("cf_{}_{}_{}",aba.id,aba.versao,lista.len()));
        let estilo = CondStyle {
            bg: Some(self.condicional_cor.clone()),
            color: if self.condicional_cor_texto.is_empty() {None}
                else {Some(self.condicional_cor_texto.clone())},
            bold: self.condicional_negrito.then_some(true),
            italic: self.condicional_italico.then_some(true),
            underline: self.condicional_sublinhado.then_some(true),
            strike: self.condicional_riscado.then_some(true),
        };
        let condicao = if cores_alternadas {
            Condition::Cell(CellCondition {
                op: CellOp::Formula, values:vec!["=ÉPAR(LIN())".into()],
                style: CondStyle {bg:Some("#f3f3f3".into()),..Default::default()},
            })
        } else {
            match self.condicional_operador {
                TipoCondicional::Escala => {
                    // Regras TypeScript podem trazer limites numéricos/percentuais.
                    // Editar as cores não deve mudar silenciosamente essas contas.
                    let anterior = editando.and_then(|i| match &lista[i].condition {
                        Condition::ColorScale(escala) => Some(escala),
                        _ => None,
                    });
                    let min = ScalePoint {
                        point_type: anterior.map_or(ScalePointType::Min, |e| e.min.point_type),
                        value: anterior.and_then(|e| e.min.value),
                        color: self.condicional_cor_min.clone(),
                    };
                    let max = ScalePoint {
                        point_type: anterior.map_or(ScalePointType::Max, |e| e.max.point_type),
                        value: anterior.and_then(|e| e.max.value),
                        color: self.condicional_cor.clone(),
                    };
                    let mid = self.condicional_ponto_meio.then(|| {
                        let anterior = anterior.and_then(|e| e.mid.as_ref());
                        ScalePoint {
                            point_type: anterior.map_or(ScalePointType::Percentile, |m| m.point_type),
                            value: anterior.and_then(|m| m.value).or(Some(50.0)),
                            color: self.condicional_cor_meio.clone(),
                        }
                    });
                    Condition::ColorScale(ColorScaleCondition {min, mid, max})
                },
                TipoCondicional::Barra => {
                    let anterior = editando.and_then(|i| match &lista[i].condition {
                        Condition::DataBar(barra) => Some(barra),
                        _ => None,
                    });
                    Condition::DataBar(DataBarCondition {
                        color:self.condicional_cor.clone(),
                        negative_color:if self.condicional_cor_negativa.is_empty(){None}
                            else{Some(self.condicional_cor_negativa.clone())},
                        hide_value:self.condicional_ocultar_valor,
                        min:anterior.and_then(|b|b.min.clone()),
                        max:anterior.and_then(|b|b.max.clone()),
                    })
                },
                tipo => {
                    let op = tipo.operador().and_then(|nome|
                        serde_json::from_value::<CellOp>(serde_json::json!(nome)).ok());
                    let Some(op) = op else {
                        self.status=tr!("Tipo de regra condicional inválido").into();return;
                    };
                    let mut values=Vec::new();
                    if tipo.aridade()>0 { values.push(self.condicional_valor.clone()); }
                    if tipo.aridade()>1 { values.push(self.condicional_valor2.clone()); }
                    Condition::Cell(CellCondition {op,values,style:estilo})
                }
            }
        };
        let ranges = if cores_alternadas || self.condicional_intervalo.trim().is_empty() {
            vec![selecao]
        } else {
            match ler_intervalos(&self.condicional_intervalo) {
                Some(v) => v,
                None => {
                    self.status = tr!("“Aplica-se a” inválido: use A1:B10, ou várias áreas separadas por ;").into();
                    return;
                }
            }
        };
        let regra = ConditionalRule {id,ranges,condition:condicao};
        if let Some(i) = editando {
            lista[i] = regra;
        } else {
            lista.push(regra);
        }
        self.condicional_editando = None;
        self.gravar_condicionais(lista);
    }

    pub(super) fn gravar_condicionais(
        &mut self, lista: Vec<feathersheet_core::operacoes::rules::ConditionalRule>
    ) {
        match condicional::gravar(&mut self.workbook,self.folha,lista) {
            Ok(()) => {
                self.alterado=true;
                self.status=tr!("Regras de formatação condicional atualizadas").into();
            }
            Err(e) => self.status=e,
        }
    }

    pub(super) fn view_grade(&self) -> Element<'_, Msg> {
        let grade = crate::compat::canvas(self.grade()).width(Length::FILL).height(Length::FILL);
        // Lista suspensa e balões (comentário, mensagem de entrada) em camada
        // própria, por cima do texto das células. Sempre empilhada: a árvore
        // de widgets não muda e a grade não perde o estado ao abrir a lista.
        let lista = crate::compat::canvas(crate::grade::lista_suspensa::CamadaLista(self.grade()))
            .width(Length::FILL).height(Length::FILL);
        pelicano::stack![grade, lista].into()
    }

    /// A opção global, menos as abas que vieram do Excel sem grade.
    pub(super) fn linhas_grade_visiveis(&self) -> bool {
        self.linhas_grade && !self.workbook.aba(self.folha).is_some_and(|a| self.grade_oculta.contains(&a.id))
    }

    pub(super) fn grade(&self) -> Grade<'_> {
        Grade {
            workbook: &self.workbook,
            cache: &self.cache_grade,
            eixos: &self.eixos_grade,
            condicional: &self.condicional,
            aba: self.folha,
            linha_inicio: self.linha_inicio,
            coluna_inicio: self.coluna_inicio,
            congelar_linhas: self.congelamento_atual().0,
            congelar_colunas: self.congelamento_atual().1,
            selecionada: (self.linha,self.coluna),
            area: self.area_selecionada(),
            // As células ficam claras no tema escuro, como no Excel: só a interface escurece.
            escuro: false,
            linhas_grade: self.linhas_grade_visiveis(),
            mostrar_formulas: self.mostrar_formulas,
            fonte_padrao: &self.config.fonte_grade,
            tamanho_fonte_padrao: self.config.tamanho_fonte_grade,
            realce_eixos: self.config.realce_eixos,
            zoom: self.config.zoom as f32 / 100.,
            ctrl_pressionado: self.ctrl_pressionado,
            rascunho: if self.editor_formula.ativo &&
                self.editor_formula.origem==Some((self.folha,self.linha,self.coluna)) {
                Some(self.edicao.as_str())
            } else {None},
            lista_aberta: self.lista_aberta,
            lista_pagina: self.lista_pagina,
            lista_destaque: self.lista_destaque,
            opcoes_coluna: self.lista_coluna.as_deref(),
            areas_extras: &self.selecoes_extras,
            impressao: self.impressao_visual(),
            copiada: self.marca_copia.as_ref().filter(|(aba, _)| *aba == self.folha).map(|(_, area)| area.clone()),
            setas: self.setas_visiveis(),
            comentarios: self.visao_comentarios(),
            circulos_validacao: self.circulos_visiveis(),
            refs_formula: if self.editor_formula.ativo && self.edicao.starts_with('=') {
                feathersheet_core::formula::assistente::reference_boxes(&self.edicao)
                    .into_iter().filter(|r|match r.sheet.as_deref() {
                        Some(nome)=>self.workbook.aba(self.folha)
                            .is_some_and(|aba|aba.sheet.name.eq_ignore_ascii_case(nome)),
                        None=>self.editor_formula.origem.is_some_and(|(origem,_,_)|origem==self.folha),
                    }).collect()
            } else {self.caixas_do_grafico()},
        }
    }

    pub(super) fn view_abas(&self) -> Element<'_, Msg> {
        let nomes = self.workbook.nomes();
        let mut linha = row![button(text("+").size(20))
            .on_press(Msg::AdicionarAba).style(estilo_botao_pequeno(false))
            .padding([1,7])].spacing(2).align(pelicano::Align::Center);
        for (i, nome) in nomes.into_iter().enumerate() {
            if self.workbook.aba(i).is_some_and(|a| a.sheet.oculta) {
                continue;
            }
            let ativo = i == self.folha;
            let cor_aba = self.workbook.aba(i)
                .and_then(|a| a.sheet.tab_color.as_deref())
                .and_then(cores_aba::cor_css);
            // SVG real em vez de glifo Unicode: funciona sem fontes extras no Linux.
            // A cor personalizada da aba continua aplicada ao fundo do botão.
            let editando = self.editando_nome_aba
                && self.workbook.aba(i).is_some_and(|aba|
                    Some(aba.id) == self.aba_em_renomeacao);
            // O nome vira um TextInput somente na guia em edição. As outras
            // continuam botões normais e toda a Ribbon permanece acessível.
            let aba: Element<'_, Msg> = if editando {
                let tamanho = ((self.nome_aba.chars().count().max(8) as f32 * 7.2) + 16.0)
                    .clamp(80.0, 240.0);
                let campo = text_input(tr!("Nome da aba"), &self.nome_aba)
                    .id("feathersheet.renomear-aba")
                    .on_input(Msg::NomearAba)
                    .on_submit(Msg::RenomearAba)
                    .size(13)
                    .padding([3, 5])
                    .width(Length::Fixed(tamanho));
                container(row![
                    feather_ui::icones::icone("table", 14.0),
                    campo,
                ].spacing(5).align(pelicano::Align::Center))
                    .padding([2, 7])
                    .style(move |_| pelicano::Style {
                        background: if let Some(mut cor) = cor_aba {
                            cor.a = 0.34;
                            Some(pelicano::Background::Color(cor))
                        } else { Some(pelicano::Background::Color(COR_AZUL_FUNDO())) },
                        border: pelicano::Border {
                            radius: [12., 12., 0., 0.],
                            ..Default::default()
                        },
                        ..Default::default()
                    }).into()
            } else {
                button(row![
                    feather_ui::icones::icone("table", 14.0),
                    text(nome).size(13),
                ].spacing(6).align(pelicano::Align::Center))
                    .style(move |_, estado| pelicano::Style {
                        background: if let Some(mut cor) = cor_aba {
                            cor.a = if ativo { 0.34 } else { 0.20 };
                            Some(pelicano::Background::Color(cor))
                        } else if ativo { Some(pelicano::Background::Color(COR_AZUL_FUNDO())) }
                            else if matches!(estado,pelicano::Status::Hovered) {
                                Some(pelicano::Background::Color(COR_PAINEL_FAIXA()))
                            } else { None },
                        text_color: Some(if ativo { COR_AZUL_ACENTO() } else { COR_TEXTO_MUTED() }),
                        border: pelicano::Border {radius:[12.,12.,0.,0.],..Default::default()},
                        ..Default::default()
                    })
                    .on_press(Msg::CliqueAba(i))
                    .padding([4,12]).into()
            };
            linha = linha.push(pelicano::mouse_area(aba)
                .on_right_press(Msg::AbrirContexto(Contexto::Aba(i), 0.0, 0.0)));
        }
        // Altura fixa: um overlay Height::Fill faria a barra de abas
        // absorver todo o espaço restante do layout em coluna.
        const ALTURA_ABAS: f32 = 34.0;
        let guias = pelicano::mouse_area(
            scrollable(linha.padding([2,5]))
                .direction(pelicano::Direction::Horizontal)
                .width(Length::FILL)
                .height(Length::Fixed(ALTURA_ABAS))
        ).on_move(|p| Msg::PonteiroSobreAbas(p.x));
        guias.into()
    }

    /// Aviso temporário do rodapé (5 s): fica AO LADO das guias, nunca por
    /// cima delas. Antes era centrado sobre a barra de abas e cobria as guias
    /// ("Aberto: /home/…" escondia as abas logo depois de abrir um arquivo).
    /// Largura limitada; texto longo perde o meio, mantendo o fim (nome do arquivo).
    pub(super) fn view_aviso_abas(&self) -> Option<Element<'_, Msg>> {
        let (mensagem, _) = self.aviso_abas.as_ref()?;
        Some(container(text(encurtar_aviso(mensagem, 80)).size(12).color(COR_TEXTO_MUTED())
                .no_wrap())
            .padding([4, 10])
            .max_width(440.0)
            .clip(true)
            .style(|_| pelicano::Style {
                background: Some(pelicano::Background::Color(COR_PAINEL_FAIXA())),
                border: pelicano::Border {
                    color: COR_TEXTO_MUTED(),
                    width: 0.5,
                    radius: [7.0; 4],
                },
                ..Default::default()
            })
            .into())
    }

    pub(super) fn view_configuracoes(&self) -> Element<'_, Msg> {
        configuracoes_ui::visualizar(self)
    }

    pub(super) fn view_ajuda(&self) -> Element<'_, Msg> {
        ajuda::modelo().view(&self.ajuda_estado).map(Msg::AjudaEvento)
    }

}

/// Encurta o aviso do rodapé: caminho de arquivo vira só o nome (o caminho
/// inteiro continua no Monitor F12) e o que ainda passar de `maximo`
/// caracteres perde o meio (`início…fim`).
pub(super) fn encurtar_aviso(mensagem: &str, maximo: usize) -> String {
    let sem_pastas = mensagem.split(" · ").map(|parte| {
        match (parte.find('/'), parte.rfind('/')) {
            // Só caminho absoluto (`/` no começo de uma palavra): datas como
            // 14/10/2026 ficam como estão.
            (Some(primeira), Some(ultima)) if primeira < ultima
                && (primeira == 0 || parte[..primeira].ends_with(' ')) =>
                format!("{}{}", &parte[..primeira], &parte[ultima + 1..]),
            _ => parte.to_owned(),
        }
    }).collect::<Vec<_>>().join(" · ");
    let total = sem_pastas.chars().count();
    if total <= maximo || maximo < 5 { return sem_pastas; }
    let inicio = maximo / 3;
    let fim = maximo - inicio - 1;
    let comeco: String = sem_pastas.chars().take(inicio).collect();
    let final_: String = sem_pastas.chars().skip(total - fim).collect();
    format!("{comeco}…{final_}")
}

#[cfg(test)]
mod testes_aviso {
    use super::encurtar_aviso;

    #[test]
    fn aviso_mostra_so_o_nome_do_arquivo_e_corta_o_meio_se_longo() {
        let msg = "Aberto: /home/usuario/Documentos/FeatherSheet/0 - Planilha de Informações -v6.0a.xlsm \u{b7} carregado em 0.1 s \u{b7} 159 MB de RAM";
        assert_eq!(encurtar_aviso(msg, 90),
            "Aberto: 0 - Planilha de Informações -v6.0a.xlsm \u{b7} carregado em 0.1 s \u{b7} 159 MB de RAM");
        let curto = encurtar_aviso(msg, 40);
        assert_eq!(curto.chars().count(), 40);
        assert!(curto.starts_with("Aberto: 0 - "));
        assert!(curto.ends_with("159 MB de RAM"));
        assert_eq!(encurtar_aviso("Aba excluída", 60), "Aba excluída");
        // Barra sem ser caminho (uma só) fica como está.
        assert_eq!(encurtar_aviso("Linhas 1/2 coladas", 60), "Linhas 1/2 coladas");
        assert_eq!(encurtar_aviso("Salvo em 14/10/2026", 60), "Salvo em 14/10/2026");
    }
}

/// "A1:B10; D2" do campo "Aplica-se a" em áreas; `None` se alguma não for endereço.
fn ler_intervalos(texto: &str) -> Option<Vec<feathersheet_core::operacoes::rules::Faixa>> {
    use feathersheet_core::operacoes::rules::Faixa;
    texto.split([';', ',']).map(str::trim).filter(|t| !t.is_empty())
        .map(|t| feathersheet_core::celulas::refs::parse_range_ref(t)
            .map(|a| Faixa { r0: a.r0, c0: a.c0, r1: a.r1, c1: a.c1 }))
        .collect::<Option<Vec<_>>>()
        .filter(|v| !v.is_empty())
}
