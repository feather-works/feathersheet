//! Revisão ▸ Permitir edição de intervalos (#82), como no Excel: intervalos da aba que
//! continuam editáveis com a planilha protegida, com senha opcional.
//!
//! Os intervalos moram no `pr` da aba (JSON; o `.xlsx` leva como `<protectedRanges>`).
//! O motor não lê JSON: [`App::liberar_areas`] põe em `Sheet::areas_liberadas` os que valem
//! agora (sem senha, ou com a senha digitada nesta sessão), antes de cada ação conferida
//! pela proteção. Digitar numa célula travada de um intervalo com senha pede a senha.
use super::*;
use feathersheet_core::celulas::refs::{cell_name, parse_range_ref};
use feathersheet_core::planilha::sheet::protecao::SenhaPlanilha;
use serde_json::{json, Value as Json};

#[derive(Debug, Clone, PartialEq)]
pub(super) struct Intervalo {
    pub id: String,
    pub descricao: String,
    pub area: Rect,
    /// `None`: sem senha. A senha de formato desconhecido (o PBKDF2 do app antigo) também
    /// pede senha, mas não confere: fica travada.
    pub senha: Option<Option<SenhaPlanilha>>,
}

pub(super) fn ler(pr: Option<&str>) -> Vec<Intervalo> {
    let lista: Vec<Json> = pr.and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
    lista.iter().filter_map(|i| {
        let a = &i["area"];
        let n = |k: &str| a[k].as_u64().and_then(|v| u32::try_from(v).ok());
        let s = &i["senha"];
        let senha = (!s.is_null()).then(|| (|| Some(SenhaPlanilha {
            algoritmo: s["alg"].as_str()?.into(), hash: s["hash"].as_str()?.into(),
            sal: s["sal"].as_str()?.into(), giros: u32::try_from(s["giros"].as_u64()?).ok()?,
        }))());
        Some(Intervalo {
            id: i["id"].as_str().unwrap_or("").to_owned(),
            descricao: i["descricao"].as_str().unwrap_or("").to_owned(),
            area: Rect::new(n("r0")?, n("c0")?, n("r1")?, n("c1")?),
            senha,
        })
    }).collect()
}

/// Grava a lista de volta, mantendo o JSON original dos itens que não mudaram (senha antiga).
fn gravar(pr: Option<&str>, lista: &[Intervalo]) -> Option<String> {
    if lista.is_empty() { return None; }
    let antigos: Vec<Json> = pr.and_then(|t| serde_json::from_str(t).ok()).unwrap_or_default();
    let itens: Vec<Json> = lista.iter().map(|i| {
        antigos.iter().find(|a| a["id"].as_str() == Some(i.id.as_str())).cloned().unwrap_or_else(|| json!({
            "id": i.id, "descricao": i.descricao,
            "area": { "r0": i.area.r0, "c0": i.area.c0, "r1": i.area.r1, "c1": i.area.c1 },
            "senha": i.senha.as_ref().and_then(|s| s.as_ref()).map(feathersheet_io::para_workbook::senha_para_json),
        }))
    }).collect();
    serde_json::to_string(&itens).ok()
}

#[derive(Debug, Clone)]
pub enum Evento {
    Abrir,
    Fechar,
    Descricao(String),
    Area(String),
    Senha(String),
    Adicionar,
    Excluir(String),
    /// A caixa da senha pedida ao digitar numa célula de intervalo com senha.
    SenhaPedida(String),
    SenhaOk,
    SenhaCancelar,
}

#[derive(Debug, Clone, Default)]
pub(super) struct Estado {
    descricao: String,
    area: String,
    senha: String,
    erro: Option<String>,
}

/// A senha pedida: (id da aba, id do intervalo, digitada, erro).
pub(super) type Pedido = (u32, String, String, Option<String>);

impl App {
    /// Põe em cada aba protegida as áreas que a proteção deixa editar agora.
    pub(super) fn liberar_areas(&mut self) {
        for i in 0..self.workbook.len() {
            let Some(aba) = self.workbook.aba(i) else { continue };
            if !aba.sheet.protegida() && aba.sheet.areas_liberadas.is_empty() { continue; }
            let id_aba = aba.id;
            let areas: Vec<Rect> = ler(aba.sheet.pr.as_deref()).into_iter()
                .filter(|it| it.senha.is_none() || self.intervalos_abertos.contains(&(id_aba, it.id.clone())))
                .map(|it| it.area).collect();
            if aba.sheet.areas_liberadas != areas {
                // Só estado da sessão: não passa pelo desfazer nem marca alteração.
                if let Some(a) = self.workbook.aba_mut(i) { a.sheet.areas_liberadas = areas; }
            }
        }
    }

    /// Digitar numa célula travada de um intervalo com senha: abre a caixa da senha.
    pub(super) fn pedir_senha_do_intervalo(&mut self) -> bool {
        let Some(aba) = self.workbook.aba(self.folha) else { return false };
        let (id_aba, r, c) = (aba.id, self.linha, self.coluna);
        let Some(it) = ler(aba.sheet.pr.as_deref()).into_iter()
            .find(|it| it.senha.is_some() && it.area.contains(r, c)) else { return false };
        self.senha_intervalo = Some((id_aba, it.id, String::new(), None));
        true
    }

    pub(super) fn evento_intervalos(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Abrir => {
                let a = self.area_selecionada();
                let area = if a.r0 == a.r1 && a.c0 == a.c1 { cell_name(a.r0, a.c0) }
                    else { format!("{}:{}", cell_name(a.r0, a.c0), cell_name(a.r1, a.c1)) };
                let n = self.workbook.aba(self.folha).map_or(0, |s| ler(s.sheet.pr.as_deref()).len()) + 1;
                self.intervalos_dialogo = Some(Estado { descricao: trf!("Intervalo {n}"), area, ..Default::default() });
            }
            Evento::Fechar => self.intervalos_dialogo = None,
            Evento::Descricao(t) => if let Some(e) = &mut self.intervalos_dialogo { e.descricao = t; },
            Evento::Area(t) => if let Some(e) = &mut self.intervalos_dialogo { e.area = t; e.erro = None; },
            Evento::Senha(t) => if let Some(e) = &mut self.intervalos_dialogo { e.senha = t; },
            Evento::Adicionar => {
                let Some(e) = self.intervalos_dialogo.clone() else { return Task::none() };
                let erro = |app: &mut Self, m: &str| { if let Some(e) = &mut app.intervalos_dialogo { e.erro = Some(m.into()); } };
                if self.workbook.aba(self.folha).is_some_and(|a| a.sheet.protegida()) {
                    erro(self, tr!("Desproteja a planilha para mudar os intervalos."));
                    return Task::none();
                }
                let Some(area) = parse_range_ref(e.area.trim().trim_start_matches('=')) else {
                    erro(self, tr!("Intervalo inválido: use algo como B2:D10."));
                    return Task::none();
                };
                let senha = if e.senha.is_empty() { None } else {
                    match protecao_ui::criar_senha(&e.senha) {
                        Ok(s) => Some(Some(s)),
                        Err(m) => { erro(self, &m); return Task::none(); }
                    }
                };
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                let mut lista = ler(aba.sheet.pr.as_deref());
                let id = format!("i{}", (1..).find(|n| !lista.iter().any(|i| i.id == format!("i{n}"))).unwrap_or(1));
                let descricao = if e.descricao.trim().is_empty() { tr!("Intervalo").to_owned() } else { e.descricao.trim().to_owned() };
                lista.push(Intervalo { id, descricao, area, senha });
                let novo = gravar(aba.sheet.pr.as_deref(), &lista);
                self.workbook.editar(self.folha, |s| s.pr = novo);
                self.alterado = true;
                let n = lista.len() + 1;
                self.intervalos_dialogo = Some(Estado { descricao: trf!("Intervalo {n}"), area: e.area, ..Default::default() });
                self.status = tr!("Intervalo liberado: vale quando a planilha for protegida.").into();
            }
            Evento::Excluir(id) => {
                let Some(aba) = self.workbook.aba(self.folha) else { return Task::none() };
                if aba.sheet.protegida() {
                    if let Some(e) = &mut self.intervalos_dialogo { e.erro = Some(tr!("Desproteja a planilha para mudar os intervalos.").into()); }
                    return Task::none();
                }
                let mut lista = ler(aba.sheet.pr.as_deref());
                lista.retain(|i| i.id != id);
                let novo = gravar(aba.sheet.pr.as_deref(), &lista);
                self.workbook.editar(self.folha, |s| s.pr = novo);
                self.alterado = true;
            }
            Evento::SenhaPedida(t) => if let Some(p) = &mut self.senha_intervalo { p.2 = t; p.3 = None; },
            Evento::SenhaCancelar => self.senha_intervalo = None,
            Evento::SenhaOk => {
                let Some((id_aba, id, digitada, _)) = self.senha_intervalo.clone() else { return Task::none() };
                let Some(aba) = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).find(|a| a.id == id_aba) else { return Task::none() };
                let Some(it) = ler(aba.sheet.pr.as_deref()).into_iter().find(|i| i.id == id) else { return Task::none() };
                let certa = match &it.senha {
                    Some(Some(s)) => protecao_ui::conferir(&digitada, s),
                    _ => Err(tr!("Este intervalo tem uma senha de formato antigo, que o FeatherSheet não confere.").into()),
                };
                match certa {
                    Ok(true) => {
                        self.intervalos_abertos.insert((id_aba, id));
                        self.senha_intervalo = None;
                        self.liberar_areas();
                        self.status = trf!("Intervalo \"{}\" liberado até fechar o arquivo.", it.descricao);
                    }
                    Ok(false) => if let Some(p) = &mut self.senha_intervalo { p.3 = Some(tr!("Senha incorreta.").into()); },
                    Err(m) => if let Some(p) = &mut self.senha_intervalo { p.3 = Some(m); },
                }
            }
        }
        Task::none()
    }

    fn view_intervalos(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let e = self.intervalos_dialogo.as_ref().expect("diálogo de intervalos aberto");
        let m = |ev: Evento| Msg::Intervalos(ev);
        let aba = self.workbook.aba(self.folha);
        let protegida = aba.is_some_and(|a| a.sheet.protegida());
        let lista = aba.map(|a| ler(a.sheet.pr.as_deref())).unwrap_or_default();
        let rotulo = |t: &str| text(t.to_owned()).size(12).color(COR_TEXTO_MUTED());
        let mut itens = pelicano::column(Vec::<Element<'_, Msg>>::new()).spacing(4);
        for i in &lista {
            let area = format!("{}:{}", cell_name(i.area.r0, i.area.c0), cell_name(i.area.r1, i.area.c1));
            itens = itens.push(row![
                text(i.descricao.clone()).size(13).width(Length::FILL),
                text(area).size(12).font(pelicano::Font::MONO),
                text(if i.senha.is_some() { tr!("com senha") } else { tr!("sem senha") }).size(11).color(COR_TEXTO_MUTED()),
                button(text(tr!("Excluir")).size(12)).style(estilo_botao_secundario)
                    .on_press_maybe((!protegida).then(|| m(Evento::Excluir(i.id.clone())))),
            ].spacing(10).align(pelicano::Align::Center));
        }
        if lista.is_empty() {
            itens = itens.push(rotulo(tr!("Nenhum intervalo ainda.")));
        }
        let mut corpo = pelicano::column![
            rotulo(tr!("Com a planilha protegida, estes intervalos continuam editáveis (com a senha, quando houver).")),
            itens,
            Space::new(0.0, 6),
            rotulo(tr!("Novo intervalo")),
            text_input(tr!("Título"), &e.descricao).on_input(move |t| m(Evento::Descricao(t))).padding(8).size(13),
            text_input("B2:D10", &e.area).on_input(move |t| m(Evento::Area(t))).padding(8).size(13),
            text_input(tr!("Senha (opcional)"), &e.senha).secure(true).on_input(move |t| m(Evento::Senha(t)))
                .on_submit(m(Evento::Adicionar)).padding(8).size(13),
        ].spacing(6);
        if let Some(erro) = &e.erro {
            corpo = corpo.push(text(erro.clone()).size(12).color(COR_ERRO()));
        }
        self.dialogo(tr!("Permitir edição de intervalos"), corpo)
            .icone("lock-open")
            .largura(520.0)
            .ao_fechar(m(Evento::Fechar))
            .secundario(tr!("Fechar"), Some(m(Evento::Fechar)))
            .primario(tr!("Adicionar"), (!protegida).then(|| m(Evento::Adicionar)))
    }

    fn view_senha_intervalo(&self) -> super::dialogo_interno::Dialogo<'_, Msg> {
        let (_, _, digitada, erro) = self.senha_intervalo.as_ref().expect("senha pedida");
        let m = |ev: Evento| Msg::Intervalos(ev);
        let mut corpo = pelicano::column![
            text(tr!("A célula está num intervalo protegido por senha. Digite a senha para editar.")).size(13),
            text_input(tr!("Senha"), digitada).secure(true).on_input(move |t| m(Evento::SenhaPedida(t)))
                .on_submit(m(Evento::SenhaOk)).padding(8).size(13),
        ].spacing(8);
        if let Some(erro) = erro {
            corpo = corpo.push(text(erro.clone()).size(12).color(COR_ERRO()));
        }
        self.dialogo(tr!("Desbloquear intervalo"), corpo)
            .icone("lock")
            .largura(420.0)
            .ao_fechar(m(Evento::SenhaCancelar))
            .secundario(tr!("Cancelar"), Some(m(Evento::SenhaCancelar)))
            .primario(tr!("OK"), Some(m(Evento::SenhaOk)))
    }

    pub(super) fn sobrepor_intervalos<'a>(&'a self, base: Element<'a, Msg>) -> Element<'a, Msg> {
        if self.senha_intervalo.is_some() { return self.view_senha_intervalo().sobre(base); }
        if self.intervalos_dialogo.is_some() { return self.view_intervalos().sobre(base); }
        base
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    #[test]
    fn intervalo_com_senha_libera_so_depois_da_senha() {
        let mut app = App::default();
        app.workbook.escrever_celula(0, 0, 0, "x".into());
        // Dois intervalos: B2:C3 sem senha e E5 com senha.
        let _ = app.update(Msg::Intervalos(Evento::Abrir));
        let _ = app.update(Msg::Intervalos(Evento::Area("B2:C3".into())));
        let _ = app.update(Msg::Intervalos(Evento::Adicionar));
        let _ = app.update(Msg::Intervalos(Evento::Area("E5".into())));
        let _ = app.update(Msg::Intervalos(Evento::Senha("abc".into())));
        let _ = app.update(Msg::Intervalos(Evento::Adicionar));
        let _ = app.update(Msg::Intervalos(Evento::Fechar));
        assert_eq!(ler(app.workbook.aba(0).unwrap().sheet.pr.as_deref()).len(), 2);
        app.workbook.editar(0, |s| { s.proteger(feathersheet_core::planilha::sheet::protecao::ProtecaoPlanilha {
            senha: None, permitidas: feathersheet_core::planilha::sheet::protecao::Permissao::padrao() }); });
        app.liberar_areas();
        let s = &app.workbook.aba(0).unwrap().sheet;
        assert!(!s.celula_bloqueada(1, 1), "sem senha: livre");
        assert!(s.celula_bloqueada(4, 4), "com senha: travada até digitar");
        assert!(s.celula_bloqueada(0, 0));
        // Digitar em E5 pede a senha; a errada não libera, a certa sim.
        app.linha = 4; app.coluna = 4;
        assert!(app.pedir_senha_do_intervalo());
        let _ = app.update(Msg::Intervalos(Evento::SenhaPedida("zzz".into())));
        let _ = app.update(Msg::Intervalos(Evento::SenhaOk));
        assert!(app.senha_intervalo.as_ref().is_some_and(|p| p.3.is_some()));
        let _ = app.update(Msg::Intervalos(Evento::SenhaPedida("abc".into())));
        let _ = app.update(Msg::Intervalos(Evento::SenhaOk));
        assert!(app.senha_intervalo.is_none());
        assert!(!app.workbook.aba(0).unwrap().sheet.celula_bloqueada(4, 4));
    }
}
