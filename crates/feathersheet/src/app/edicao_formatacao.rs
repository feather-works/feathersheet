//! Formatação, cores, comandos da Ribbon, edição e colagem especial.
use super::*;

/// A partir disto (linhas), um valor colado vai em blocos de 65.536 linhas.
const LINHAS_COLAGEM_RAPIDA: usize = 100_000;

impl App {
    pub(super) fn formatar(&mut self, acao: Formatacao) {
        self.confirmar();
        let area = self.area_selecionada();
        let estilo = self.workbook.aba(self.folha).and_then(|a|
            a.sheet.estilo_resolvido(self.linha,self.coluna)).unwrap_or_default();
        let mut patch = EstiloParcial::new();
        let (chave, valor) = match acao {
            Formatacao::Negrito => ("bold",Campo::Logico(!estilo.bold)),
            Formatacao::Italico => ("italic",Campo::Logico(!estilo.italic)),
            Formatacao::Porcentagem => ("format", Campo::Texto(if estilo.format == NumberFormat::Percent {
                "auto" } else { "percent" }.into())),
            Formatacao::Moeda => ("format", Campo::Texto(if estilo.format == NumberFormat::Currency {
                "auto" } else { "currency" }.into())),
            Formatacao::Esquerda => ("align",Campo::Texto("left".into())),
            Formatacao::Centro => ("align",Campo::Texto("center".into())),
            Formatacao::Direita => ("align",Campo::Texto("right".into())),
            Formatacao::Limpar => {
                self.workbook.editar(self.folha,|sheet| {
                    sheet.clear(&area);
                });
                self.alterado=true;
                self.trocar_selecao(self.linha,self.coluna);
                self.status=tr!("Célula e formato limpos").into();
                return;
            }
        };
        patch.insert(chave.into(),valor);
        self.workbook.editar(self.folha,|sheet| sheet.set_style(&area,&patch));
        self.alterado=true;
        self.status=trf!("Formatação aplicada em {}",cell_name(self.linha,self.coluna));
    }

    pub(super) fn cor_da_celula(&self, alvo: AlvoCor) -> String {
        self.workbook.aba(self.folha)
            .and_then(|a| a.sheet.estilo_resolvido(self.linha, self.coluna))
            .and_then(|e| match alvo { AlvoCor::Texto => e.color, AlvoCor::Fundo => e.bg })
            .unwrap_or_default()
    }

    pub(super) fn aplicar_cor(&mut self, valor: String) -> Task<Msg> {
        let Some(alvo) = self.cor_ativa else { return Task::none(); };
        if !valor.is_empty() {
            self.cor_hsva = seletor_cor::rgba_para_hsva(
                seletor_cor::rgba_de(&valor), self.cor_hsva[0]);
            self.cor_hex = valor.clone();
        }
        let chave = match alvo { AlvoCor::Texto => "color", AlvoCor::Fundo => "bg" };
        let campo = if valor.is_empty() { Campo::Nulo } else { Campo::Texto(valor) };
        self.update(Msg::EstiloMenu(chave.into(), campo))
    }

    pub(super) fn atualizar_seletor_cor(&mut self, evento: EventoCor) -> Task<Msg> {
        if self.cor_ativa.is_none() { return Task::none(); }
        match evento {
            EventoCor::Fechar => {
                self.cor_ativa = None;
                Task::none()
            }
            EventoCor::Escolhida(hex) => self.aplicar_cor(hex),
            EventoCor::SaturacaoBrilho(s, v) => {
                self.cor_hsva[1] = s.clamp(0., 1.);
                self.cor_hsva[2] = v.clamp(0., 1.);
                self.aplicar_cor(self.hex_do_seletor())
            }
            EventoCor::Matiz(h) => {
                self.cor_hsva[0] = h.rem_euclid(360.);
                self.aplicar_cor(self.hex_do_seletor())
            }
            EventoCor::Alfa(alpha) => {
                self.cor_hsva[3] = alpha.clamp(0., 1.);
                self.aplicar_cor(self.hex_do_seletor())
            }
            EventoCor::TextoHex(valor) => {
                self.cor_hex = valor.clone();
                if seletor_cor::hex_valido(&valor) {
                    self.aplicar_cor(format!("#{}", valor.trim().trim_start_matches('#').to_lowercase()))
                } else { Task::none() }
            }
            EventoCor::Canal(indice, valor) => {
                let Ok(n) = valor.trim().parse::<u16>() else { return Task::none(); };
                if indice > 3 || n > 255 { return Task::none(); }
                let mut canais = seletor_cor::rgba_de(&self.hex_do_seletor());
                canais[indice] = n as u8;
                self.aplicar_cor(seletor_cor::hex_de(canais))
            }
        }
    }

    pub(super) fn hex_do_seletor(&self) -> String {
        let [h, s, v, a] = self.cor_hsva;
        let [r, g, b] = seletor_cor::hsv_para_rgb(h, s, v);
        seletor_cor::hex_de([r, g, b, (a * 255.).round() as u8])
    }

    /// Seletores da Ribbon compartilham os mesmos comandos das opções do menu.
    pub(super) fn faixa_escolha(&mut self, grupo: &'static str, opcao: &'static str) -> Task<Msg> {
        match grupo {
            "validacao" => self.update(Msg::ValidacaoAbrir),
            "fonte" => {
                let familia = match opcao {
                    "Padrão" => return self.update(Msg::EstiloMenu("fontFamily".into(), Campo::Nulo)),
                    "Serifada" => "'IBM Plex Serif', Georgia, serif",
                    "Monoespaçada" => "'IBM Plex Mono', monospace",
                    "Arial" => "Arial, Helvetica, sans-serif",
                    "Georgia" => "Georgia, serif",
                    "Courier New" => "'Courier New', monospace",
                    "Verdana" => "Verdana, sans-serif",
                    "Times New Roman" => "'Times New Roman', serif",
                    _ => return Task::none(),
                };
                self.update(Msg::EstiloMenu("fontFamily".into(), Campo::Texto(familia.into())))
            }
            "tamanho" => match opcao.parse::<u32>() {
                Ok(valor) if (6..=96).contains(&valor) =>
                    self.update(Msg::EstiloMenu("fontSize".into(), Campo::Numero(valor as f64))),
                _ => Task::none(),
            }
            "formato" => {
                if opcao == "Personalizado…" {
                    return self.abrir_formatar_celulas(formatar_celulas::Aba::Numero);
                }
                let valor = match opcao {
                    "Geral" | "Automático" => return self.update(Msg::EstiloMenu("format".into(), Campo::Nulo)),
                    "1.234,56" => "number", "1.235" => "integer",
                    "R$ 1.234,56" => "currency", "12,34%" => "percent",
                    "Data" => "date", "Data e hora" => "datetime",
                    "Hora" => "time", "Texto" => "text",
                    _ => return Task::none(),
                };
                self.update(Msg::EstiloMenu("format".into(), Campo::Texto(valor.into())))
            }
            "categoria" => {
                if opcao == "Todas" {
                    self.categoria_funcoes = 0;
                } else if let Some(i) = feathersheet_core::funcoes::function_help::CATEGORIES
                    .iter().position(|(nome,_)| *nome==opcao) {
                    self.categoria_funcoes=i+1;
                }
                Task::none()
            }
            "funcao" => self.update(Msg::InserirFuncao(opcao.into())),
            "total_tabela" => self.acao_tabela(tabelas::Acao::Funcao(opcao)),
            "cor-texto" | "cor-fundo" => {
                let cor = match opcao {
                    "Preto" => "#1f1f1f", "Branco" => "#ffffff",
                    "Vermelho" => "#d93025", "Verde" => "#188038",
                    "Azul" => "#0b57d0", "Amarelo" => "#fff2b2",
                    "Laranja" => "#e8710a", "Cinza" => "#808080",
                    "Sem cor" => return self.update(Msg::EstiloMenu(
                        (if grupo == "cor-texto" { "color" } else { "bg" }).into(), Campo::Nulo)),
                    _ => return Task::none(),
                };
                self.update(Msg::EstiloMenu(
                    (if grupo == "cor-texto" { "color" } else { "bg" }).into(),
                    Campo::Texto(cor.into())))
            }
            "borda" => {
                self.bordas_menu_aberto = false;
                use std::collections::BTreeMap;
                let mut lados=BTreeMap::new();
                let direcoes: &[&str] = match opcao {
                    "Todas" | "Externas" => &["top", "bottom", "left", "right"],
                    "Horizontais internas" => &["top", "bottom"],
                    "Verticais internas" => &["left", "right"],
                    "Superior" => &["top"], "Inferior" => &["bottom"],
                    "Esquerda" => &["left"], "Direita" => &["right"],
                    "Nenhuma" => &[], _ => return Task::none(),
                };
                for &lado in direcoes { lados.insert(lado.into(), Campo::Logico(true)); }
                self.update(Msg::EstiloMenu("borders".into(),
                    if direcoes.is_empty() { Campo::Nulo } else { Campo::Objeto(lados) }))
            }
            "linhas" => self.update(match opcao {
                "Inserir acima" => Msg::InserirMenu(Estrutura::InserirLinha,false),
                "Inserir abaixo" => Msg::InserirMenu(Estrutura::InserirLinha,true),
                "Excluir linhas" => Msg::Estrutura(Estrutura::ExcluirLinha),
                _ => return Task::none(),
            }),
            "colunas" => self.update(match opcao {
                "Inserir à esquerda" => Msg::InserirMenu(Estrutura::InserirColuna,false),
                "Inserir à direita" => Msg::InserirMenu(Estrutura::InserirColuna,true),
                "Excluir colunas" => Msg::Estrutura(Estrutura::ExcluirColuna),
                _ => return Task::none(),
            }),
            "dividir_texto" => {
                let padrao = self.regionalizacao().formula_arg_separator.to_string();
                let separador = match opcao {
                    "Idioma" => padrao.as_str(),
                    ";" => ";",
                    "," => ",",
                    "." => ".",
                    "Espaço" => " ",
                    "Outro…" => {
                        self.divisao_personalizada = true;
                        return Task::none();
                    }
                    _ => return Task::none(),
                };
                self.divisao_personalizada = false;
                self.dividir_texto(separador);
                Task::none()
            }
            "tema" => match opcao {
                "Automático" | "Auto" => self.update(Msg::TemaAuto),
                "Claro" => self.update(Msg::Tema(false)),
                "Escuro" => self.update(Msg::Tema(true)),
                _ => Task::none(),
            },
            "zoom" => match opcao.trim_end_matches('%').parse::<u16>() {
                Ok(p) if (25..=400).contains(&p) => self.update(Msg::Zoom(p)),
                _ => Task::none(),
            },
            _ => Task::none(),
        }
    }

    pub(super) fn aplicar_estilo_menu(&mut self, chave: &str, valor: Campo) {
        self.confirmar();
        let areas = self.areas_selecionadas();
        let mut patch = EstiloParcial::new();
        patch.insert(chave.into(), valor);
        // Escolher um formato da lista desfaz o código personalizado.
        if chave == "format" { patch.insert("numFmt".into(), Campo::Nulo); }
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for area in &areas { sheet.set_style(area, &patch); }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        self.status = tr!("Formatação atualizada").into();
    }

    pub(super) fn alternar_estilo_menu(&mut self, chave: &str) {
        use feathersheet_core::operacoes::sublinhado::{alternar_sublinhado, Sublinhado, CHAVE_DUPLO};
        // Sublinhado simples e duplo se excluem (motor), como no Excel.
        if chave == "underline" || chave == CHAVE_DUPLO {
            self.confirmar();
            let tipo = if chave == CHAVE_DUPLO { Sublinhado::Duplo } else { Sublinhado::Simples };
            let areas = self.areas_selecionadas();
            let ativa = (self.linha, self.coluna);
            let mut novo = Sublinhado::Nenhum;
            self.workbook.editar(self.folha, |sheet| {
                novo = alternar_sublinhado(sheet, &areas, ativa, tipo);
            });
            self.alterado = true;
            self.status = match novo {
                Sublinhado::Duplo => tr!("Sublinhado duplo aplicado"),
                Sublinhado::Simples => tr!("Sublinhado aplicado"),
                Sublinhado::Nenhum => tr!("Sublinhado removido"),
            }.into();
            return;
        }
        let estilo = self.workbook.aba(self.folha)
            .and_then(|aba| aba.sheet.estilo_resolvido(self.linha, self.coluna))
            .unwrap_or_default();
        let atual = match chave {
            "bold" => estilo.bold,
            "italic" => estilo.italic,
            "underline" => estilo.underline,
            "strike" => estilo.strike,
            _ => return,
        };
        self.aplicar_estilo_menu(chave, Campo::Logico(!atual));
    }

    pub(super) fn inserir_menu(&mut self, acao: Estrutura, depois: bool) {
        self.confirmar();
        let (coluna, at) = match acao {
            Estrutura::InserirLinha => (false, self.linha.saturating_add(u32::from(depois))),
            Estrutura::InserirColuna => (true, self.coluna.saturating_add(u32::from(depois))),
            _ => return,
        };
        let resultado = if coluna {
            self.workbook.inserir_colunas(self.folha, at, 1)
        } else {
            self.workbook.inserir_linhas(self.folha, at, 1)
        };
        if resultado.is_some() {
            let tabela = tabelas::refluir(&mut self.workbook, self.folha, coluna, at, 1, false);
            let mescla = mesclas::refluir(&mut self.workbook, self.folha, coluna, at, 1, false);
            // Tabelas e mesclas reajustadas também guardam o "antes": um Ctrl+Z
            // as poria nas coordenadas velhas, com as células já deslocadas (#46).
            if let Some(aba) = self.workbook.aba_mut(self.folha) { aba.sheet.esquecer_historico(); }
            self.alterado = true;
            self.trocar_selecao(self.linha, self.coluna);
            self.status = match (tabela, mescla) {
                (Ok(()), Ok(())) => tr!("Estrutura, tabelas e mesclagens atualizadas").into(),
                (Err(e), Ok(())) => trf!("Estrutura alterada; tabela requer revisão: {e}"),
                (Ok(()), Err(e)) => trf!("Estrutura alterada; mesclagem requer revisão: {e}"),
                (Err(a), Err(b)) => trf!("Estrutura alterada; tabela: {a}; mesclagem: {b}"),
            };
        } else {
            self.status = tr!("Não foi possível inserir nesse local").into();
        }
    }

    pub(super) fn estrutura(&mut self, acao: Estrutura) {
        self.confirmar();
        let resultado = match acao {
            Estrutura::InserirLinha => self.workbook.inserir_linhas(self.folha,self.linha,1),
            Estrutura::ExcluirLinha => self.workbook.excluir_linhas(self.folha,self.linha,1),
            Estrutura::InserirColuna => self.workbook.inserir_colunas(self.folha,self.coluna,1),
            Estrutura::ExcluirColuna => self.workbook.excluir_colunas(self.folha,self.coluna,1),
        };
        if resultado.is_some() {
            let (coluna,removendo)=match acao {
                Estrutura::InserirLinha=>(false,false),
                Estrutura::ExcluirLinha=>(false,true),
                Estrutura::InserirColuna=>(true,false),
                Estrutura::ExcluirColuna=>(true,true),
            };
            let pos=if coluna {self.coluna} else {self.linha};
            let tabela=tabelas::refluir(&mut self.workbook,self.folha,coluna,pos,1,removendo);
            let mescla=mesclas::refluir(&mut self.workbook,self.folha,coluna,pos,1,removendo);
            // Tabelas e mesclas reajustadas também guardam o "antes": um Ctrl+Z
            // as poria nas coordenadas velhas, com as células já deslocadas (#46).
            if let Some(aba) = self.workbook.aba_mut(self.folha) { aba.sheet.esquecer_historico(); }
            self.alterado=true;
            self.trocar_selecao(self.linha,self.coluna);
            self.status=match (tabela, mescla) {
                (Ok(()),Ok(()))=>tr!("Estrutura, tabelas e mesclagens atualizadas").into(),
                (Err(e),Ok(()))=>trf!("Estrutura alterada; tabela requer revisão: {e}"),
                (Ok(()),Err(e))=>trf!("Estrutura alterada; mesclagem requer revisão: {e}"),
                (Err(a),Err(b))=>trf!("Estrutura alterada; tabela: {a}; mesclagem: {b}"),
            };
        } else {
            self.status=tr!("Não foi possível alterar a estrutura").into();
        }
    }

    pub(super) fn limpar_celula(&mut self) {
        let Some(aba) = self.workbook.aba(self.folha) else { return };
        let usado = aba.sheet.used_rect();
        // Limpar conteúdo não precisa percorrer bilhões de células vazias.
        let areas: Vec<Rect> = self.areas_selecionadas().into_iter()
            .map(|a| Rect::new(a.r0, a.c0, a.r1.min(usado.r1), a.c1.min(usado.c1)))
            .filter(|a| a.r0 <= a.r1 && a.c0 <= a.c1).collect();
        // Caixa de seleção (Excel): Delete desmarca; com todas as caixas da
        // seleção já desmarcadas, tira as caixas.
        let caixas = crate::caixas::Regras::da(&aba.sheet);
        let selecao = self.areas_selecionadas();
        let tem_caixa = selecao.iter().any(|a| caixas.alguma_caixa_em(&a.normalized()));
        let todas_desmarcadas = tem_caixa && areas.iter().all(|a| (a.r0..=a.r1).all(|r| (a.c0..=a.c1).all(|c|
            caixas.caixa_em(r, c).is_none_or(|(m, d)| crate::caixas::estado(&aba.sheet, r, c, m, d) != Some(true)))));
        let sem_caixas = todas_desmarcadas.then(|| caixas.sem_caixas_nas(&selecao)).flatten();
        if areas.is_empty() && sem_caixas.is_none() { return; }
        let principal = self.area_selecionada();
        // Todas as áreas numa transação: um Ctrl+Z desfaz tudo.
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            for area in &areas {
                for r in area.r0..=area.r1 {
                    for c in area.c0..=area.c1 {
                        match caixas.caixa_em(r, c) {
                            Some((_, d)) if sem_caixas.is_none() => {
                                if sheet.get_raw(r, c) != d { sheet.set_cell(r, c, d); }
                            }
                            _ => if !sheet.get_raw(r,c).is_empty() { sheet.limpar_conteudo(r,c); },
                        }
                    }
                }
            }
            if let Some(json) = sem_caixas.clone() {
                sheet.alterar_validacoes(Some(json), &principal);
            }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        self.edicao.clear();
    }

    /// Colagem de matriz TSV ou seleção interna (incluindo estilos).
    /// Uma célula de destino recebe o bloco inteiro. Uma área retangular
    /// selecionada é preenchida por repetição exata, numa única transação.
    pub(super) fn colar(&mut self, dados: String) {
        self.colar_modo(dados, ModoColagem::Completa);
    }

    pub(super) fn colar_modo(&mut self, dados: String, modo: ModoColagem) {
        if !self.confirmar() { return; }
        // O texto do clipboard serve como chave da cópia rica local: dados
        // externos em TSV nunca herdam acidentalmente estilos anteriores.
        let copia = self.area_copiada.as_ref().filter(|copia| copia.texto == dados);
        let internas = copia.map(|copia| &copia.celulas);
        let valores_internos = copia.map(|copia| &copia.valores);
        // Fórmula copiada no FeatherSheet anda com o destino (`=A1` de B1
        // colada em B5 vira `=A5`), como no Excel; recortada vai como está.
        // Até 17/10/2026 o texto ia igual para qualquer lugar.
        let desloca = copia.filter(|c| !c.recortada).map(|c| c.origem);
        let deslocar = |raw: &str, r: u32, c: u32, or: usize, oc: usize| -> String {
            match desloca {
                Some((r0, c0)) if raw.starts_with('=') => feathersheet_core::operacoes::fill::shift_formula(
                    raw, r as i32 - (r0 + or as u32) as i32, c as i32 - (c0 + oc as u32) as i32),
                _ => raw.to_owned(),
            }
        };
        if dados.is_empty() && internas.is_none() { return; }
        if matches!(modo, ModoColagem::SomenteFormato | ModoColagem::ValoresComFormato)
            && internas.is_none()
        {
            self.status = tr!("Esta colagem com formatação requer células copiadas do FeatherSheet").into();
            return;
        }
        let com_formatacao = internas.is_some()
            && matches!(modo, ModoColagem::Completa | ModoColagem::Transposta
                | ModoColagem::ValoresComFormato);
        let externas = if internas.is_none() {
            Some(feathersheet_io::csv::dividir_csv(&dados, '\t'))
        } else { None };
        let (origem_altura, origem_largura) = if let Some(linhas) = internas {
            (linhas.len(), linhas.iter().map(Vec::len).max().unwrap_or(0))
        } else {
            let linhas = externas.as_ref().expect("texto externo");
            (linhas.len(), linhas.iter().map(Vec::len).max().unwrap_or(0))
        };
        let (altura_bloco, largura_bloco) = if modo == ModoColagem::Transposta {
            (origem_largura, origem_altura)
        } else {
            (origem_altura, origem_largura)
        };
        // Um valor numa coluna até o fim (A1 em A2:A100000000, 17/10/2026):
        // a parte vazia da coluna vai em blocos pelo motor, então cola tudo.
        // `Some` = o texto e o estilo de cada coluna do bloco (uma linha só,
        // sem validação na aba); data e moeda levam o formato numa faixa.
        let selecao = self.area_selecionada().normalized();
        let rapida: Option<Vec<(String, Option<feathersheet_core::celulas::estilo_parcial::EstiloParcial>)>> = (|| {
            if origem_altura != 1 || matches!(modo, ModoColagem::Transposta
                | ModoColagem::SomenteFormato | ModoColagem::ValoresComFormato) {
                return None;
            }
            if ((selecao.r1 - selecao.r0) as usize) < LINHAS_COLAGEM_RAPIDA { return None; }
            // Seleção gigante (Ctrl+A e colar uma linha de fórmulas): vai pelo caminho comum,
            // que para na última linha/coluna com dados.
            let celulas = u64::from(selecao.r1 - selecao.r0 + 1) * u64::from(selecao.c1 - selecao.c0 + 1);
            if celulas > super::preenchimento_ui::TETO_CELULAS_EM_BLOCO { return None; }
            let validacao = self.workbook.aba(self.folha).and_then(|a| a.sheet.va.as_deref())
                .is_some_and(|t| !matches!(t.trim(), "" | "[]"));
            if validacao { return None; }
            (0..origem_largura).map(|oc| {
                let c = selecao.c0 + oc as u32;
                // O estilo que o caminho célula a célula daria à primeira linha.
                let (raw, estilo) = match (internas, modo) {
                    (Some(l), ModoColagem::Completa) => {
                        let s = l[0].get(oc)?;
                        (s.raw.clone(), s.style.clone())
                    }
                    (Some(l), ModoColagem::FormulasSemFormato) => (l[0].get(oc)?.raw.clone(), None),
                    (Some(_), _) => (valores_internos?.first()?.get(oc)?.clone(), None),
                    (None, _) => {
                        let raw = externas.as_ref()?.first()?.get(oc).cloned().unwrap_or_default();
                        let estilo = if modo == ModoColagem::Completa {
                            self.workbook.aba(self.folha)?.sheet.style_for(selecao.r0, c, &raw)
                        } else { None };
                        (raw, estilo)
                    }
                };
                // Fórmula copiada aqui vai em bloco pela coluna empacotada
                // (`preencher_formula_faixa_vazia`); a de fora ou recortada
                // seria o mesmo texto em todas as linhas, não um padrão.
                if raw.starts_with('=') {
                    let no_topo = deslocar(&raw, selecao.r0, c, 0, oc);
                    let cabe = self.workbook.aba(self.folha)
                        .is_some_and(|a| a.sheet.formula_cabe_em_bloco(c, selecao.r0, &no_topo));
                    return (desloca.is_some() && cabe).then_some((raw, estilo));
                }
                feathersheet_core::planilha::sheet::Sheet::valor_de_faixa(&raw, estilo.as_ref()).map(|_| (raw, estilo))
            }).collect()
        })();
        // Fora disso, a seleção que vai até o fim da planilha (coluna ou
        // linha inteira, Ctrl+Shift+↓ numa coluna vazia) para na última
        // linha/coluna com dados: fórmula a fórmula seriam 100 milhões de
        // células (16/10/2026).
        let (destino, cortada) = if rapida.is_some() { (selecao, false) }
            else { self.area_de_colagem(altura_bloco, largura_bloco) };
        // Validação completa antes de alterar qualquer célula.
        let plano = match PlanoColagem::calcular(
            &destino, altura_bloco, largura_bloco,
        ) {
            Ok(plano) => plano,
            Err(mensagem) => {
                self.status = mensagem.into();
                return;
            }
        };

        // Pré-validação atômica: um único valor recusado cancela o bloco inteiro.
        // Faz o parse do JSON apenas UMA vez, independentemente do tamanho.
        let mut aviso_validacao: Option<String> = None;
        if modo != ModoColagem::SomenteFormato {
            let json = self.workbook.aba(self.folha)
                .and_then(|a| a.sheet.va.as_deref());
            let regras = match validacao_ui::carregar_regras(json) {
                Ok(v) => v,
                Err(erro) => { self.status = erro; return; }
            };
            if !regras.is_empty() {
                for dr in 0..plano.altura {
                    for dc in 0..plano.largura {
                        let (or, oc) = if modo == ModoColagem::Transposta {
                            (dc % origem_altura, dr % origem_largura)
                        } else {
                            (dr % origem_altura, dc % origem_largura)
                        };
                        let raw = if matches!(modo,
                            ModoColagem::SomenteValores | ModoColagem::ValoresComFormato)
                        {
                            valores_internos.and_then(|l| l.get(or).and_then(|row| row.get(oc)))
                                .map(String::as_str)
                                .or_else(|| externas.as_ref().and_then(|l|
                                    l.get(or).and_then(|row| row.get(oc)).map(String::as_str)))
                                .unwrap_or("")
                        } else {
                            internas.and_then(|l| l.get(or).and_then(|row| row.get(oc)))
                                .map(|s| s.raw.as_str())
                                .or_else(|| externas.as_ref().and_then(|l|
                                    l.get(or).and_then(|row| row.get(oc)).map(String::as_str)))
                                .unwrap_or("")
                        };
                        let r = plano.r0 + dr as u32;
                        let c = plano.c0 + dc as u32;
                        match validacao_ui::validar_celula(
                            &self.workbook, self.folha, r, c, raw, &regras)
                        {
                            Ok(Some(aviso)) if aviso_validacao.is_none() => {
                                aviso_validacao = Some(format!("{}: {}", cell_name(r, c), aviso));
                            }
                            Ok(_) => {}
                            Err(erro) => {
                                self.status = trf!("Colagem cancelada em {}: {}",
                                    cell_name(r, c), erro);
                                return;
                            }
                        }
                    }
                }
            }
        }
        let mut sem_bloco: Option<u32> = None;
        self.workbook.editar(self.folha, |sheet| {
            sheet.abrir_transacao();
            // Com a colagem rápida, célula a célula só até a última célula
            // usada de cada coluna; a parte vazia abaixo vai em bloco, no
            // fim. Era até a última linha da aba: colar ao lado de 13,7
            // milhões de linhas de dados ia célula a célula (17/10/2026).
            let ate = plano.r0 + plano.altura as u32 - 1;
            let inicio_bloco: Vec<Option<u32>> = (0..plano.largura).map(|dc| {
                rapida.as_ref()?;
                let c = plano.c0 + dc as u32;
                let de = plano.r0.max(sheet.ultima_linha_na_coluna(c));
                (de <= ate && sheet.faixa_vazia(c, de, ate)).then_some(de)
            }).collect();
            let altura_celulas = inicio_bloco.iter()
                .map(|i| i.map_or(plano.altura, |de| (de - plano.r0) as usize))
                .max().unwrap_or(0);
            for dr in 0..altura_celulas {
                for dc in 0..plano.largura {
                    let (r, c) = (plano.r0 + dr as u32, plano.c0 + dc as u32);
                    if inicio_bloco[dc].is_some_and(|de| r >= de) { continue; }
                    // Na transposição, linhas da origem tornam-se colunas;
                    // a repetição usa as dimensões já transpostas.
                    let (or, oc) = if modo == ModoColagem::Transposta {
                        (dc % origem_altura, dr % origem_largura)
                    } else {
                        (dr % origem_altura, dc % origem_largura)
                    };
                    match modo {
                        ModoColagem::Completa | ModoColagem::Transposta => {
                            if let Some(linhas) = internas {
                                let celula = linhas[or].get(oc);
                                let destino = celula
                                    .filter(|s| !s.raw.is_empty() || s.style.is_some())
                                    .map(|s| Snap { raw: deslocar(&s.raw, r, c, or, oc), style: s.style.clone() });
                                sheet.apply(r, c, destino, true);
                            } else if let Some(linhas) = externas.as_ref() {
                                let conteudo = linhas[or].get(oc).map(String::as_str).unwrap_or("");
                                sheet.set_cell(r, c, conteudo);
                            }
                        }
                        ModoColagem::SomenteValores
                        | ModoColagem::ValoresComFormato
                        | ModoColagem::FormulasSemFormato => {
                            let deslocada;
                            let valor = if let Some(linhas) = internas {
                                if modo == ModoColagem::FormulasSemFormato {
                                    deslocada = deslocar(linhas[or].get(oc).map(|s| s.raw.as_str()).unwrap_or(""), r, c, or, oc);
                                    deslocada.as_str()
                                } else {
                                    valores_internos
                                        .and_then(|linhas| linhas[or].get(oc))
                                        .map(String::as_str).unwrap_or("")
                                }
                            } else {
                                externas.as_ref().and_then(|linhas| linhas[or].get(oc))
                                    .map(String::as_str).unwrap_or("")
                            };
                            let estilo = if modo == ModoColagem::ValoresComFormato {
                                internas.and_then(|linhas| linhas[or].get(oc))
                                    .and_then(|s| s.style.clone())
                            } else {
                                // Sem formatação significa não levar o estilo
                                // da origem; manter o estilo atual do destino.
                                sheet.snapshot(r, c).and_then(|s| s.style)
                            };
                            let destino = if valor.is_empty() && estilo.is_none() {
                                None
                            } else {
                                Some(Snap { raw: valor.to_owned(), style: estilo })
                            };
                            sheet.apply(r, c, destino, true);
                        }
                        ModoColagem::SomenteFormato => {
                            // Mantém conteúdo e fórmula atuais, trocando apenas o estilo.
                            let estilo = internas.and_then(|linhas| linhas[or].get(oc))
                                .and_then(|s| s.style.clone());
                            let mut destino = sheet.snapshot(r, c).unwrap_or_else(|| Snap::texto(""));
                            destino.style = estilo;
                            let destino = if destino.raw.is_empty() && destino.style.is_none() {
                                None
                            } else { Some(destino) };
                            sheet.apply(r, c, destino, true);
                        }
                    }
                }
            }
            if let Some(textos) = &rapida {
                for dc in 0..plano.largura {
                    let c = plano.c0 + dc as u32;
                    let oc = dc % origem_largura;
                    let (texto, estilo) = &textos[oc];
                    let de = inicio_bloco[dc].unwrap_or(plano.r0 + altura_celulas as u32);
                    let preenchida = de > ate || (sheet.faixa_vazia(c, de, ate) && if texto.starts_with('=') {
                        estilo.as_ref().is_none_or(|e| sheet.pintar_faixa_vazia(c, de, ate, e))
                            && sheet.preencher_formula_faixa_vazia(c, de, ate, &deslocar(texto, de, c, 0, oc))
                    } else {
                        sheet.preencher_faixa_vazia(c, de, ate, texto, estilo.as_ref())
                    });
                    if !preenchida { sem_bloco.get_or_insert(c); }
                }
            }
            sheet.fechar_transacao();
        });
        self.alterado = true;
        self.trocar_selecao(plano.r0, plano.c0);
        let area = plano.area();
        // Tabelas só crescem quando conteúdo é colado, e uma única vez.
        let erro_tabela = if modo != ModoColagem::SomenteFormato {
            tabelas::crescer_colagem(&mut self.workbook, self.folha, area.clone()).err()
        } else {
            None
        };
        self.fim_selecao = (area.r1, area.c1);
        if plano.altura > 1 || plano.largura > 1 {
            self.endereco = format!(
                "{}:{}", cell_name(area.r0, area.c0), cell_name(area.r1, area.c1),
            );
        }
        self.status = trf!(
            "{} células coladas{}",
            plano.altura * plano.largura,
            match modo {
                ModoColagem::Completa if com_formatacao => tr!(" com formatação"),
                ModoColagem::Completa => "",
                ModoColagem::Transposta => tr!(" (transposta)"),
                ModoColagem::SomenteValores => tr!(" (somente valores)"),
                ModoColagem::ValoresComFormato => tr!(" (valores com formatação)"),
                ModoColagem::FormulasSemFormato => tr!(" (fórmulas sem formatação)"),
                ModoColagem::SomenteFormato => tr!(" (somente formatação)"),
            },
        );
        if cortada {
            // A seleção até o fim é a escolha de quem cola; o aviso diz
            // por que parou (17/10: "não consigo colar de A2:A100.000.000").
            self.status.push_str(&trf!(" · a seleção ia até o fim da planilha (100 milhões de linhas): colado só até {}, a última linha com dados",
                cell_name(area.r1, area.c1)));
        }
        if let Some(c) = sem_bloco {
            self.status.push_str(&trf!("; a coluna {} já tinha algo abaixo da última linha usada e não foi preenchida até o fim",
                feathersheet_core::celulas::refs::col_name(c)));
        }
        if let Some(erro) = erro_tabela {
            self.status.push_str(&trf!("; não foi possível ampliar a tabela: {erro}"));
        }
        if let Some(aviso) = aviso_validacao {
            self.status.push_str(&trf!("; aviso de validação: {aviso}"));
        }
        // Por último: a pasta vai para a thread do cálculo.
        if rapida.as_ref().is_some_and(|t| t.iter().any(|(x, _)| x.starts_with('='))) {
            self.calcular_se_grande(plano.altura * plano.largura);
        }
    }

    /// A área onde colar: a seleção, mas a que chega à última linha (ou
    /// coluna) da planilha para na última com dados, arredondada para um
    /// múltiplo do bloco copiado. Planilha vazia: o bloco uma vez. Devolve
    /// também se cortou.
    pub(super) fn area_de_colagem(&self, altura: usize, largura: usize) -> (Rect, bool) {
        let mut area = self.area_selecionada().normalized();
        let Some(sheet) = self.workbook.aba(self.folha).map(|a| &a.sheet) else { return (area, false) };
        let vazia = sheet.is_empty();
        let mut cortada = false;
        let mut cortar = |inicio: u32, fim: &mut u32, maximo: u32, ultima: u32, bloco: usize| {
            if *fim < maximo - 1 || *fim == inicio || bloco == 0 {
                return;
            }
            let bloco = bloco as u32;
            let ultima = if vazia { inicio } else { ultima.max(inicio) };
            let vezes = (ultima - inicio) / bloco + 1;
            *fim = (inicio + vezes * bloco - 1).min(maximo - 1);
            cortada = true;
        };
        cortar(area.r0, &mut area.r1, MAX_ROWS, sheet.max_row, altura);
        cortar(area.c0, &mut area.c1, MAX_COLS, sheet.max_col, largura);
        (area, cortada)
    }

    pub(super) fn historico(&mut self, refazer: bool) {
        // Duplicar aba é desfeito pelo motor, quando é a edição mais recente.
        let estrutural = if refazer { self.workbook.pode_refazer_aba() }
            else { self.workbook.pode_desfazer_aba() };
        if estrutural {
            let novo = if refazer { self.workbook.refazer_aba() }
                else { self.workbook.desfazer_aba() };
            if let Some(i) = novo {
                self.folha = i;
                self.nome_aba = self.workbook.nomes().get(i).cloned().unwrap_or_default();
                self.editando_nome_aba = false;
                let (fixas_linhas,fixas_colunas)=self.congelamento_atual();
                self.linha_inicio=fixas_linhas;
                self.coluna_inicio=fixas_colunas;
                self.trocar_selecao(0,0);
                self.alterado = true;
                self.status = if refazer { tr!("Refazer") } else { tr!("Desfazer") }.into();
                return;
            }
        }
        // Só captura tb quando a próxima transação altera metadados de
        // tabela; Ctrl+Z de uma célula não deve clonar esse JSON.
        let tb_antes = self.workbook.aba(self.folha).and_then(|a|
            a.sheet.proximo_historico_altera_tabelas(refazer).then(||a.sheet.tb.clone()));
        let mut alterou = false;
        self.workbook.editar(self.folha,|sheet| {
            alterou = if refazer { sheet.redo() } else { sheet.undo() };
        });
        if alterou {
            if let Some(antes)=tb_antes {
                let depois=self.workbook.aba(self.folha)
                    .and_then(|a|a.sheet.tb.clone());
                tabelas::sincronizar_nomes_historico(
                    &mut self.workbook,self.folha,antes.as_deref(),depois.as_deref());
            }
            self.alterado = true;
            self.invalidar_cache_objetos();
            self.recarregar_grafico_aberto();
            self.trocar_selecao(self.linha,self.coluna);
            self.status = if refazer { tr!("Refazer") } else { tr!("Desfazer") }.into();
        }
    }

    /// Ajuda para a função aberta sob o cursor, sem depender do status da janela.
    /// O índice de argumento considera ; e , e chamadas aninhadas.
    pub(super) fn dica_funcao_ativa(&self) -> Option<(String, usize, String, String)> {
        if !self.editor_formula.ativo || !self.edicao.starts_with('=') { return None; }
        let chamada = feathersheet_core::formula::assistente::active_call(
            &self.edicao, self.editor_formula.posicao(&self.edicao))?;
        let ajuda = feathersheet_core::formula::assistente::ajuda(&chamada.name)?;
        let assinatura = crate::idioma::assinatura(&chamada.name, ajuda);
        let (args, doc) = crate::idioma::ajuda_traduzida(ajuda);
        let argumento = args.get(chamada.arg_index)
            .cloned().unwrap_or_else(|| tr!("argumento adicional").to_owned());
        Some((assinatura, chamada.arg_index + 1, argumento, doc))
    }

}
