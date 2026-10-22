//! Gerenciador de nomes (Fórmulas > Gerenciador), no padrão do Excel:
//! tabela Nome / Valor / Refere-se a, botões Novo, Editar e Excluir e o
//! campo "Refere-se a" editável do nome escolhido. O motor guarda os nomes
//! (`Workbook::definir_nome`); aqui só ficam o formulário e o desenho.
//! O motor não tem escopo por aba: todo nome vale para a pasta inteira.
use super::*;
use super::dialogo_interno::{dlg, Dialogo};

/// O que a parte de baixo do gerenciador mostra.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) enum Modo {
    /// Tabela + "Refere-se a" do nome escolhido.
    #[default]
    Lista,
    /// Formulário de nome novo.
    Novo,
    /// Formulário editando o nome original.
    Editar(String),
    /// Pedindo confirmação para excluir o nome.
    Excluir(String),
}

#[derive(Default)]
pub(super) struct Estado {
    pub nome: String,
    pub erro: Option<String>,
    /// Nome escolhido na tabela.
    pub selecionado: Option<String>,
    /// Texto do campo "Refere-se a" (do escolhido ou do formulário).
    pub refere: String,
    pub modo: Modo,
}

/// Ações do gerenciador além das mensagens antigas (`NomeCriar`…).
#[derive(Debug, Clone)]
pub enum Evento {
    Selecionar(String),
    Novo,
    Editar,
    Excluir,
    ExcluirConfirmar,
    Refere(String),
    /// Grava o "Refere-se a" editado do nome escolhido.
    Aplicar,
    /// Sai do formulário/confirmação e volta para a tabela.
    Voltar,
}

/// Nome de fórmula não pode ser uma referência A1 válida nem conter espaços.
/// Preserva letras acentuadas como no padrão TypeScript, com limite de 60 chars.
pub(super) fn validar_nome(nome: &str) -> Result<(), &'static str> {
    let nome = nome.trim();
    if nome.is_empty() || nome.chars().count() > 60 {
        return Err(tr!("O nome precisa ter entre 1 e 60 caracteres."));
    }
    let mut caracteres = nome.chars();
    if !caracteres.next().is_some_and(|c| c.is_alphabetic() || c == '_')
        || !caracteres.all(|c| c.is_alphanumeric() || c == '_' || c == '.')
    {
        return Err(tr!("Use letras, números, _ ou .; comece por letra ou _."));
    }
    let posicao_numero = nome.find(|c: char| c.is_ascii_digit());
    if let Some(pos) = posicao_numero {
        let coluna = &nome[..pos];
        let linha = &nome[pos..];
        if let (Some(_), Ok(linha)) = (col_index(coluna), linha.parse::<u32>()) {
            if (1..=MAX_ROWS).contains(&linha) {
                return Err(tr!("Esse nome é um endereço de célula, como A1."));
            }
        }
    }
    Ok(())
}

/// `=Planilha1!$A$1:$B$3`, como o Excel mostra em "Refere-se a".
pub(super) fn texto_refere(area: &Rect) -> String {
    let n = area.normalized();
    let aba = n.sheet.as_deref()
        .map(|a| format!("{}!", feathersheet_core::celulas::refs::quote_sheet(a)))
        .unwrap_or_default();
    let col = feathersheet_core::celulas::refs::col_name;
    let corpo = if n.is_full_col() {
        format!("${}:${}", col(n.c0), col(n.c1))
    } else if n.is_full_row() {
        format!("${}:${}", n.r0 + 1, n.r1 + 1)
    } else if n.r0 == n.r1 && n.c0 == n.c1 {
        format!("${}${}", col(n.c0), n.r0 + 1)
    } else {
        format!("${}${}:${}${}", col(n.c0), n.r0 + 1, col(n.c1), n.r1 + 1)
    };
    format!("={aba}{corpo}")
}

/// Lê "Refere-se a": `=Aba!$A$1:$B$2`, `'Meu plano'!A1`, `B7`, `C:C`.
/// Sem aba, vale a aba `padrao`. A aba precisa existir na pasta.
pub(super) fn ler_refere(
    workbook: &Workbook,
    texto: &str,
    padrao: &str,
) -> Result<Rect, &'static str> {
    let t = texto.trim();
    let t = t.strip_prefix('=').unwrap_or(t).trim();
    if t.is_empty() {
        return Err(tr!("Escreva a referência, como =Planilha1!$A$1:$B$10."));
    }
    // O ! que separa a aba é o último fora de aspas.
    let mut em_aspas = false;
    let mut separador = None;
    let mut iter = t.char_indices().peekable();
    while let Some((i, c)) = iter.next() {
        if c == '\'' {
            if em_aspas && iter.peek().is_some_and(|(_, d)| *d == '\'') {
                iter.next();
            } else {
                em_aspas = !em_aspas;
            }
        } else if c == '!' && !em_aspas {
            separador = Some(i);
        }
    }
    let (aba, endereco) = match separador {
        Some(i) => {
            let nome = &t[..i];
            let nome = if nome.len() >= 2 && nome.starts_with('\'') && nome.ends_with('\'') {
                nome[1..nome.len() - 1].replace("''", "'")
            } else {
                nome.to_owned()
            };
            (nome, &t[i + 1..])
        }
        None => (padrao.to_owned(), t),
    };
    let Some(indice) = workbook.indice_por_nome(&aba) else {
        return Err(tr!("A aba dessa referência não existe."));
    };
    let real = workbook.nomes().get(indice).cloned().unwrap_or(aba);
    let limpo: String = endereco.chars().filter(|c| *c != '$').collect();
    let Some(mut area) = feathersheet_core::celulas::refs::parse_range_ref(&limpo) else {
        return Err(tr!("Referência inválida. Use, por exemplo, =Planilha1!$A$1:$B$10."));
    };
    area.sheet = Some(real);
    Ok(area)
}

/// Corta o texto em `max` caracteres com reticências.
fn cortar(texto: &str, max: usize) -> String {
    if texto.chars().count() <= max {
        texto.to_owned()
    } else {
        let mut s: String = texto.chars().take(max.saturating_sub(1)).collect();
        s.push('…');
        s
    }
}

impl App {
    /// Seleção exatamente igual a um nome definido: a caixa de nome mostra
    /// o nome, como no Excel. Enquanto o usuário digita, nada.
    pub(super) fn nome_da_selecao(&self) -> Option<String> {
        if self.endereco_digitado { return None; }
        let aba = &self.workbook.aba(self.folha)?.sheet.name;
        let area = self.area_selecionada().normalized();
        self.workbook.nomeados().into_iter().find(|n| {
            let a = n.area.normalized();
            n.area.sheet.as_ref().is_none_or(|s| s.eq_ignore_ascii_case(aba))
                && (a.r0, a.c0, a.r1, a.c1) == (area.r0, area.c0, area.r1, area.c1)
        }).map(|n| n.nome)
    }

    pub(super) fn abrir_nomes(&mut self, preencher: bool) -> Task<Msg> {
        if !self.confirmar() { return Task::none(); }
        self.menu_aberto = false;
        self.contexto = None;
        let existentes = self.workbook.nomeados();
        let mut estado = Estado::default();
        if preencher {
            let mut n = existentes.len() + 1;
            while existentes.iter().any(|e| e.nome.to_uppercase() == format!("Intervalo{n}").to_uppercase()) {
                n += 1;
            }
            estado.nome = format!("Intervalo{n}");
            estado.modo = Modo::Novo;
            estado.refere = texto_refere(&self.area_selecionada_na_aba());
        } else if let Some(primeiro) = existentes.first() {
            estado.selecionado = Some(primeiro.nome.clone());
            estado.refere = texto_refere(&primeiro.area);
        }
        self.nomes_modal = Some(estado);
        Task::none()
    }

    /// A seleção com o nome da aba atual (o que um nome novo aponta).
    fn area_selecionada_na_aba(&self) -> Rect {
        let mut area = self.area_selecionada();
        area.sheet = self.workbook.aba(self.folha).map(|a| a.sheet.name.clone());
        area
    }

    /// Cria (modo Novo ou Lista) ou regrava (modo Editar) um nome. Sem
    /// "Refere-se a" escrito, usa a seleção. Não edita células.
    pub(super) fn criar_nome(&mut self) -> Task<Msg> {
        let Some((nome, refere, modo)) = self.nomes_modal.as_ref()
            .map(|e| (e.nome.trim().to_owned(), e.refere.clone(), e.modo.clone())) else {
            return Task::none();
        };
        let erro = |app: &mut App, msg: &str| {
            if let Some(e) = &mut app.nomes_modal { e.erro = Some(msg.into()); }
            Task::none()
        };
        if let Err(e) = validar_nome(&nome) { return erro(self, e); }
        let original = match &modo { Modo::Editar(o) => Some(o.clone()), _ => None };
        let duplicado = self.workbook.nomeados().iter().any(|e| {
            e.nome.to_uppercase() == nome.to_uppercase()
                && original.as_ref().is_none_or(|o| o.to_uppercase() != nome.to_uppercase())
        });
        if duplicado {
            return erro(self, tr!("Esse nome já existe. Escolha outro."));
        }
        let area = if matches!(modo, Modo::Lista) || refere.trim().is_empty() {
            self.area_selecionada_na_aba()
        } else {
            let padrao = self.nome_aba.clone();
            match ler_refere(&self.workbook, &refere, &padrao) {
                Ok(a) => a,
                Err(e) => return erro(self, e),
            }
        };
        if let Some(o) = &original {
            if o.to_uppercase() != nome.to_uppercase() {
                self.workbook.remover_nome(o);
            }
        }
        self.workbook.definir_nome(&nome, area.clone());
        self.alterado = true;
        self.status = if original.is_some() {
            trf!("Nome \"{nome}\" alterado.")
        } else {
            trf!("Intervalo \"{nome}\" criado.")
        };
        self.endereco = nome.clone();
        // Volta para a tabela com o nome criado escolhido.
        if let Some(e) = &mut self.nomes_modal {
            e.modo = Modo::Lista;
            e.erro = None;
            e.selecionado = Some(nome);
            e.refere = texto_refere(&area);
            e.nome.clear();
        }
        Task::none()
    }

    pub(super) fn excluir_nome(&mut self, nome: &str) -> Task<Msg> {
        if self.workbook.remover_nome(nome) {
            self.alterado = true;
            self.status = trf!("Intervalo \"{nome}\" removido.");
            if self.endereco.to_uppercase() == nome.to_uppercase() {
                self.endereco = cell_name(self.linha, self.coluna);
            }
            if let Some(e) = &mut self.nomes_modal {
                if e.selecionado.as_ref().is_some_and(|s| s.to_uppercase() == nome.to_uppercase()) {
                    e.selecionado = None;
                    e.refere.clear();
                }
            }
        }
        Task::none()
    }

    pub(super) fn ir_para_nome(&mut self, nome: &str) -> Task<Msg> {
        let Some(alvo) = self.workbook.nomeados().into_iter()
            .find(|e| e.nome.to_uppercase() == nome.to_uppercase()) else {
            self.status = trf!("O intervalo \"{nome}\" não existe.");
            return Task::none();
        };
        let folha = if let Some(nome_aba) = alvo.area.sheet.as_deref() {
            let Some(i) = self.workbook.nomes().iter()
                .position(|s| s.to_lowercase() == nome_aba.to_lowercase()) else {
                self.status = trf!("A aba do intervalo \"{nome}\" não existe.");
                return Task::none();
            };
            i
        } else {
            self.folha
        };
        if alvo.area.r1 >= MAX_ROWS || alvo.area.c1 >= MAX_COLS {
            self.status = tr!("Esse intervalo excede os limites da planilha.").into();
            return Task::none();
        }
        if !self.confirmar() { return Task::none(); }
        self.folha = folha;
        self.nome_aba = self.workbook.nomes()[folha].clone();
        let area = alvo.area.normalized();
        self.trocar_selecao(area.r0, area.c0);
        self.fim_selecao = (area.r1, area.c1);
        self.endereco = alvo.nome.clone();
        self.nomes_modal = None;
        self.status = trf!("Selecionado: {}", alvo.nome);
        Task::none()
    }

    pub(super) fn evento_nomes(&mut self, evento: Evento) -> Task<Msg> {
        let nomes = self.workbook.nomeados();
        let achar = |n: &str| nomes.iter().find(|e| e.nome.to_uppercase() == n.to_uppercase()).cloned();
        let selecao = self.area_selecionada_na_aba();
        let Some(e) = &mut self.nomes_modal else { return Task::none() };
        match evento {
            Evento::Selecionar(nome) => {
                if let Some(n) = achar(&nome) {
                    e.refere = texto_refere(&n.area);
                    e.selecionado = Some(n.nome);
                    e.modo = Modo::Lista;
                    e.erro = None;
                }
            }
            Evento::Novo => {
                let mut n = nomes.len() + 1;
                while nomes.iter().any(|x| x.nome.to_uppercase() == format!("INTERVALO{n}")) { n += 1; }
                e.nome = format!("Intervalo{n}");
                e.refere = texto_refere(&selecao);
                e.modo = Modo::Novo;
                e.erro = None;
            }
            Evento::Editar => {
                if let Some(n) = e.selecionado.as_deref().and_then(achar) {
                    e.nome = n.nome.clone();
                    e.refere = texto_refere(&n.area);
                    e.modo = Modo::Editar(n.nome);
                    e.erro = None;
                }
            }
            Evento::Excluir => {
                if let Some(n) = e.selecionado.clone() { e.modo = Modo::Excluir(n); }
            }
            Evento::ExcluirConfirmar => {
                if let Modo::Excluir(nome) = std::mem::take(&mut e.modo) {
                    let _ = self.excluir_nome(&nome);
                    let primeiro = self.workbook.nomeados().into_iter().next();
                    if let (Some(e), Some(p)) = (&mut self.nomes_modal, primeiro) {
                        e.selecionado = Some(p.nome);
                        e.refere = texto_refere(&p.area);
                    }
                }
            }
            Evento::Refere(texto) => {
                e.refere = texto;
                e.erro = None;
            }
            Evento::Aplicar => {
                let Some(nome) = e.selecionado.clone() else { return Task::none() };
                let refere = e.refere.clone();
                let padrao = self.nome_aba.clone();
                match ler_refere(&self.workbook, &refere, &padrao) {
                    Ok(area) => {
                        let grafia = achar(&nome).map(|n| n.nome).unwrap_or(nome);
                        self.workbook.definir_nome(&grafia, area.clone());
                        self.alterado = true;
                        self.status = trf!("Nome \"{grafia}\" agora se refere a {}.", texto_refere(&area));
                        if let Some(e) = &mut self.nomes_modal { e.refere = texto_refere(&area); }
                    }
                    Err(msg) => {
                        if let Some(e) = &mut self.nomes_modal { e.erro = Some(msg.into()); }
                    }
                }
            }
            Evento::Voltar => {
                e.modo = Modo::Lista;
                e.erro = None;
                if let Some(n) = e.selecionado.as_deref().and_then(achar) {
                    e.refere = texto_refere(&n.area);
                } else {
                    e.refere.clear();
                }
            }
        }
        Task::none()
    }

    /// O que a coluna Valor mostra: o valor da célula, ou `{a; b; …}`.
    fn valor_do_nome(&self, area: &Rect) -> String {
        let Some(aba) = area.sheet.as_deref().and_then(|a| self.workbook.indice_por_nome(a)) else {
            return "#REF!".into();
        };
        let n = area.normalized();
        // Número com vírgula decimal, como a grade mostra em pt-BR.
        let texto = |r, c| match self.workbook.valor(aba, r, c) {
            feathersheet_core::Value::Number(n) =>
                feathersheet_core::celulas::values::to_text(&feathersheet_core::Value::Number(n)).replace('.', ","),
            v => feathersheet_core::celulas::values::to_text(&v),
        };
        if n.r0 == n.r1 && n.c0 == n.c1 {
            return texto(n.r0, n.c0);
        }
        let ultima = self.workbook.aba(aba).map(|a| a.sheet.used_rect().r1).unwrap_or(0);
        let mut itens = Vec::new();
        'fora: for r in n.r0..=n.r1.min(ultima) {
            for c in n.c0..=n.c1.min(n.c0 + 20) {
                itens.push(texto(r, c));
                if itens.len() == 4 { break 'fora; }
            }
        }
        let total = u64::from(n.r1 - n.r0 + 1) * u64::from(n.c1 - n.c0 + 1);
        let mais = if (itens.len() as u64) < total { "; …" } else { "" };
        format!("{{{}{mais}}}", itens.join("; "))
    }

    fn tabela_nomes(&self, estado: &Estado) -> Element<'_, Msg> {
        let nomes = self.workbook.nomeados();
        if nomes.is_empty() {
            return dlg::aviso_vazio(tr!("Nenhum nome definido. Use Novo para dar um nome à seleção atual."));
        }
        let semibold = pelicano::Font { weight: 600, ..Default::default() };
        let cabecalho = |t: &str| text(t.to_uppercase()).size(10.5).font(semibold).color(COR_TEXTO_MUTED());
        let titulo = container(row![
            cabecalho(tr!("Nome")).width(Length::Fill(3)),
            cabecalho(tr!("Valor")).width(Length::Fill(3)),
            cabecalho(tr!("Refere-se a")).width(Length::Fill(4)),
        ].spacing(10)).padding([4, 10]);
        let mut linhas = pelicano::column(Vec::<pelicano::Element<_>>::new()).spacing(1);
        for n in nomes {
            let ativo = estado.selecionado.as_ref().is_some_and(|s| s.to_uppercase() == n.nome.to_uppercase());
            let linha = row![
                text(cortar(&n.nome, 22)).size(12.5).width(Length::Fill(3)),
                text(cortar(&self.valor_do_nome(&n.area), 24)).size(12.5).color(COR_TEXTO_MUTED())
                    .width(Length::Fill(3)),
                text(cortar(&texto_refere(&n.area), 30)).size(12.5).width(Length::Fill(4)),
            ].spacing(10).align(pelicano::Align::Center);
            linhas = linhas.push(
                button(linha).width(Length::FILL).padding([6, 10])
                    .style(estilo_botao_pequeno(ativo))
                    .on_press(Msg::NomesEvento(Evento::Selecionar(n.nome))),
            );
        }
        container(pelicano::column![
            titulo,
            container(Space::new(Length::FILL, Length::Fixed(1.0))).style(estilo_linha),
            container(dlg::rolagem(linhas)).max_height(232.0),
        ].spacing(2))
            .padding(4).width(Length::FILL).style(dlg::estilo_bloco).into()
    }

    fn view_nomes(&self) -> Dialogo<'_, Msg> {
        let estado = self.nomes_modal.as_ref().expect("gerenciador de nomes aberto");
        let tem = estado.selecionado.is_some();
        let acao = |icone: &str, rotulo: &str, msg: Option<Msg>| -> Element<'_, Msg> {
            button(row![feather_ui::icones::icone(icone, 15.0), text(rotulo.to_owned()).size(12.5)]
                .spacing(6).align(pelicano::Align::Center))
                .padding([5, 10]).style(estilo_botao_secundario).on_press_maybe(msg).into()
        };
        let em_lista = matches!(estado.modo, Modo::Lista);
        let barra = row![
            acao("plus", tr!("Novo…"), Some(Msg::NomesEvento(Evento::Novo))),
            acao("edit", tr!("Editar…"), (tem && em_lista).then_some(Msg::NomesEvento(Evento::Editar))),
            acao("trash", tr!("Excluir"), (tem && em_lista).then_some(Msg::NomesEvento(Evento::Excluir))),
        ].spacing(6);

        let rotulo = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        let erro: Option<Element<'_, Msg>> = estado.erro.as_ref().map(|e| {
            row![feather_ui::icones::icone("info-circle", 15.0), text(e.clone()).size(12).color(COR_ERRO())]
                .spacing(6).align(pelicano::Align::Center).into()
        });

        let mut corpo = pelicano::column![barra, self.tabela_nomes(estado)].spacing(10);
        let d = match &estado.modo {
            Modo::Lista => {
                let mut campo = text_input(tr!("Escolha um nome na tabela"), &estado.refere).padding(8).size(13);
                if tem {
                    campo = campo.on_input(|t| Msg::NomesEvento(Evento::Refere(t)))
                        .on_submit(Msg::NomesEvento(Evento::Aplicar));
                }
                corpo = corpo.push(pelicano::column![rotulo(tr!("Refere-se a")), campo].spacing(5));
                if let Some(e) = erro { corpo = corpo.push(e); }
                let atual = estado.selecionado.as_ref().and_then(|s| {
                    self.workbook.nomeados().into_iter().find(|n| n.nome.to_uppercase() == s.to_uppercase())
                }).map(|n| texto_refere(&n.area));
                let mudou = atual.is_some_and(|a| a != estado.refere.trim());
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .rodape_esquerda(dlg::botao_secundario(tr!("Ir para"),
                        estado.selecionado.clone().map(Msg::NomeIrPara)))
                    .primario(tr!("Aplicar"), mudou.then_some(Msg::NomesEvento(Evento::Aplicar)))
            }
            Modo::Novo | Modo::Editar(_) => {
                let novo = matches!(estado.modo, Modo::Novo);
                let form = pelicano::column![
                    dlg::secao(if novo { tr!("Novo nome") } else { tr!("Editar nome") }),
                    rotulo(tr!("Nome")),
                    text_input(tr!("Ex.: receitas_2026"), &estado.nome)
                        .on_input(Msg::NomeAtualizar).on_submit(Msg::NomeCriar).padding(8).size(13),
                    rotulo(tr!("Refere-se a")),
                    text_input("=Planilha1!$A$1:$B$10", &estado.refere)
                        .on_input(|t| Msg::NomesEvento(Evento::Refere(t)))
                        .on_submit(Msg::NomeCriar).padding(8).size(13),
                ].spacing(6);
                corpo = corpo.push(container(form).padding([10, 12]).width(Length::FILL).style(dlg::estilo_bloco));
                if let Some(e) = erro { corpo = corpo.push(e); }
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .secundario(tr!("Cancelar"), Some(Msg::NomesEvento(Evento::Voltar)))
                    .primario(if novo { tr!("Criar nome") } else { tr!("Salvar") }, Some(Msg::NomeCriar))
            }
            Modo::Excluir(nome) => {
                corpo = corpo.push(dlg::aviso("alert-triangle", trf!(
                    "Excluir o nome \"{nome}\"? As fórmulas que o usam passam a mostrar #NOME?.")));
                self.dialogo(tr!("Gerenciador de nomes"), corpo)
                    .secundario(tr!("Cancelar"), Some(Msg::NomesEvento(Evento::Voltar)))
                    .primario(tr!("Excluir nome"), Some(Msg::NomesEvento(Evento::ExcluirConfirmar)))
            }
        };
        d.icone("tag")
            .subtitulo(tr!("Use os nomes em fórmulas, como =SOMA(receitas). Valem para a pasta inteira."))
            .largura(640.0)
            .ao_fechar(Msg::NomesFechar)
    }

    pub(super) fn sobrepor_nomes<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.nomes_modal.is_none() { return base; }
        self.view_nomes().sobre(base)
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn valida_nomes_como_o_typescript_sem_confundir_enderecos() {
        for nome in ["Receitas", "preços_2026", "_base.2025"] {
            assert!(validar_nome(nome).is_ok(), "{nome}");
        }
        for nome in ["", "A1", "XFD10000000", "A1:B2", "com espaço", "12abc", "a/b"] {
            assert!(validar_nome(nome).is_err(), "{nome}");
        }
    }

    #[test]
    fn caixa_de_nome_mostra_o_nome_da_selecao_e_uma_celula_sem_repetir() {
        let mut app = App::default();
        app.workbook.definir_nome("teste1", Rect::na_aba(16, 6, 16, 6, "Planilha1"));
        app.trocar_selecao(16, 6);
        assert_eq!(app.nome_da_selecao().as_deref(), Some("teste1"));
        app.fim_selecao = (17, 6);
        assert_eq!(app.nome_da_selecao(), None, "G17:G18 não é o nome");
        // #21/#26: Shift+clique de volta na mesma célula escrevia `G17:G17`.
        let _ = app.update(Msg::Estender(16, 6));
        assert_eq!(app.endereco, "G17");
        let _ = app.update(Msg::Estender(18, 7));
        assert_eq!(app.endereco, "G17:H19");
    }

    #[test]
    fn cria_navega_e_remove_sem_alterar_celulas() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 1, 2, "123".into());
        app.linha = 1;
        app.coluna = 2;
        app.fim_selecao = (3, 4);
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("Receitas".into()));
        app.update(Msg::NomeCriar);
        assert_eq!(app.workbook.nomeados()[0].area, Rect::na_aba(1, 2, 3, 4, "Planilha1"));
        assert!(app.alterado);
        app.linha = 0;
        app.coluna = 0;
        app.fim_selecao = (0, 0);
        app.update(Msg::IrPara("receitas".into()));
        assert_eq!((app.linha, app.coluna, app.fim_selecao), (1, 2, (3, 4)));
        assert_eq!(app.workbook.aba(0).unwrap().sheet.get_raw(1, 2), "123");
        app.update(Msg::NomeRemover("RECEITAS".into()));
        assert!(app.workbook.nomeados().is_empty());
    }

    #[test]
    fn gerenciador_recusa_nome_duplicado_e_referencia_em_outra_aba() {
        let mut app = App::default();
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("Base".into()));
        app.update(Msg::NomeCriar);
        app.update(Msg::NomesAbrir(false));
        app.update(Msg::NomeAtualizar("base".into()));
        app.update(Msg::NomeCriar);
        assert!(app.nomes_modal.as_ref().unwrap().erro.is_some());
        assert_eq!(app.workbook.nomeados().len(), 1);
        app.update(Msg::NomesFechar);
        app.workbook.definir_nome("Invalido", Rect::na_aba(0,0,0,0,"Excluida"));
        app.update(Msg::IrPara("Invalido".into()));
        assert!(app.status.contains("não existe"));
    }

    #[test]
    fn refere_se_a_ida_e_volta() {
        let wb = Workbook::new();
        let aba = wb.nomes()[0].clone();
        let r = Rect::na_aba(0, 0, 9, 1, aba.clone());
        assert_eq!(texto_refere(&r), format!("={aba}!$A$1:$B$10"));
        assert_eq!(ler_refere(&wb, &texto_refere(&r), &aba).unwrap(), r);
        assert_eq!(ler_refere(&wb, "B7", &aba).unwrap(), Rect::na_aba(6, 1, 6, 1, aba.clone()));
        assert_eq!(texto_refere(&Rect::na_aba(6, 1, 6, 1, aba.clone())), format!("={aba}!$B$7"));
        assert!(ler_refere(&wb, "=Outra!A1", &aba).is_err());
        assert!(ler_refere(&wb, "=", &aba).is_err());
        assert!(ler_refere(&wb, "zz", &aba).is_err());
    }

    #[test]
    fn novo_editar_aplicar_e_excluir_com_confirmacao() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "5".into());
        app.update(Msg::NomesAbrir(true));
        assert_eq!(app.nomes_modal.as_ref().unwrap().modo, Modo::Novo);
        assert_eq!(app.nomes_modal.as_ref().unwrap().refere, "=Planilha1!$A$1");
        app.update(Msg::NomeAtualizar("Taxa".into()));
        app.update(Msg::NomeCriar);
        let e = app.nomes_modal.as_ref().unwrap();
        assert_eq!((e.modo.clone(), e.selecionado.as_deref()), (Modo::Lista, Some("Taxa")));
        assert_eq!(app.valor_do_nome(&app.workbook.nomeados()[0].area), "5");
        app.workbook.escrever_celula(0, 0, 0, "0,12".into());
        assert_eq!(app.valor_do_nome(&app.workbook.nomeados()[0].area), "0,12");
        let _: Element<'_, Msg> = app.view_nomes().painel();

        // "Refere-se a" editado na lista.
        app.update(Msg::NomesEvento(Evento::Refere("=Planilha1!$A$1:$A$3".into())));
        app.update(Msg::NomesEvento(Evento::Aplicar));
        assert_eq!(app.workbook.nomeados()[0].area, Rect::na_aba(0, 0, 2, 0, "Planilha1"));

        // Editar renomeia.
        app.update(Msg::NomesEvento(Evento::Editar));
        app.update(Msg::NomeAtualizar("Juros".into()));
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.update(Msg::NomeCriar);
        let nomes: Vec<String> = app.workbook.nomeados().into_iter().map(|n| n.nome).collect();
        assert_eq!(nomes, vec!["Juros".to_string()]);

        // Excluir pede confirmação.
        app.update(Msg::NomesEvento(Evento::Excluir));
        assert_eq!(app.workbook.nomeados().len(), 1);
        let _: Element<'_, Msg> = app.view_nomes().painel();
        app.update(Msg::NomesEvento(Evento::ExcluirConfirmar));
        assert!(app.workbook.nomeados().is_empty());
        let _: Element<'_, Msg> = app.view_nomes().painel();
    }
}
