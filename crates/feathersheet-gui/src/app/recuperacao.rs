//! Recuperação automática, como a do Excel: acabou a luz, travou, o PC
//! desligou — a planilha alterada (mesmo a que nunca foi salva) volta.
//!
//! Enquanto há alteração sem salvar, uma cópia vai para
//! `Documentos/FeatherSheet/Temporarios/Recuperacao/<pid>.fshz` em segundo
//! plano, poucos segundos depois de a pessoa parar de digitar. Ao lado, um
//! `<pid>.json` com o nome do documento e um pulso renovado enquanto o app
//! vive. Salvar, descartar ou fechar normalmente apaga os dois. Na abertura,
//! cópia cujo pulso parou é de uma sessão que morreu: a tela inicial oferece
//! recuperar ou descartar.
use super::*;
use std::time::SystemTime;

/// Espera depois da última edição antes de copiar (não grava no meio da digitação).
const OCIOSO: Duration = Duration::from_secs(3);
/// Intervalo mínimo entre cópias (sobe sozinho para planilhas grandes).
const INTERVALO_MS: u64 = 20_000;
/// O pulso do `.json` é renovado nesse ritmo enquanto o app está aberto.
const PULSO: Duration = Duration::from_secs(10);
/// Sem pulso há mais que isso, a sessão morreu.
const SEM_PULSO_S: u64 = 45;

#[derive(Debug, Clone)]
pub enum Evento {
    Gravada(Result<u64, String>),
    Recuperar(usize),
    Descartar(usize),
}

#[derive(Debug, Clone)]
pub(super) struct Orfa {
    pub fshz: PathBuf,
    pub json: PathBuf,
    pub nome: String,
    pub origem: Option<PathBuf>,
    pub quando: SystemTime,
    pub tamanho: u64,
}

#[derive(Default)]
pub(super) struct Estado {
    pub(super) gravando: bool,
    /// Versões das abas na última cópia: sem mudança, não grava de novo.
    versoes: Option<Vec<(u32, u32)>>,
    ultima: Option<Instant>,
    pulso: Option<Instant>,
    custo_ms: u64,
    /// Cópias de sessões que morreram, oferecidas na tela inicial.
    pub orfas: Vec<Orfa>,
    /// Origem do documento recuperado que está abrindo (vira o nome sugerido).
    pub origem_pendente: Option<Option<PathBuf>>,
}

pub(super) fn pasta() -> PathBuf {
    // Os testes nunca tocam na pasta de Documentos da pessoa.
    if cfg!(test) {
        return std::env::temp_dir().join(format!("feathersheet-teste-recuperacao-{}", std::process::id()));
    }
    App::pasta_temporarios().join("Recuperacao")
}
fn minhas() -> (PathBuf, PathBuf) {
    let id = std::process::id();
    (pasta().join(format!("{id}.fshz")), pasta().join(format!("{id}.json")))
}
fn agora_s() -> u64 {
    SystemTime::now().duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs())
}

/// As cópias que ficaram de sessões mortas, a mais nova primeiro.
pub(super) fn procurar_orfas() -> Vec<Orfa> {
    let meu = std::process::id();
    let Ok(entradas) = std::fs::read_dir(pasta()) else { return Vec::new() };
    let mut orfas: Vec<Orfa> = entradas.filter_map(Result::ok).filter_map(|e| {
        let json = e.path();
        if json.extension().and_then(|x| x.to_str()) != Some("json") {
            return None;
        }
        let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).ok()?).ok()?;
        if meta["pid"].as_u64() == Some(u64::from(meu)) {
            return None;
        }
        // Pulso recente: outra janela do FeatherSheet está viva com ela.
        if agora_s().saturating_sub(meta["pulso"].as_u64().unwrap_or(0)) < SEM_PULSO_S {
            return None;
        }
        let fshz = json.with_extension("fshz");
        let dados = std::fs::metadata(&fshz).ok()?;
        Some(Orfa {
            nome: meta["nome"].as_str().unwrap_or(tr!("Sem título")).to_owned(),
            origem: meta["origem"].as_str().map(PathBuf::from),
            quando: dados.modified().unwrap_or(SystemTime::UNIX_EPOCH),
            tamanho: dados.len(),
            fshz, json,
        })
    }).collect();
    orfas.sort_by(|a, b| b.quando.cmp(&a.quando));
    orfas
}

fn apagar(fshz: &Path, json: &Path) {
    let _ = std::fs::remove_file(fshz);
    let _ = std::fs::remove_file(json);
}

impl App {
    /// A cópia desta sessão não serve mais (salvou, descartou ou trocou de
    /// documento): apaga.
    pub(super) fn descartar_recuperacao(&mut self) {
        let (fshz, json) = minhas();
        apagar(&fshz, &json);
        self.recuperacao.versoes = None;
        self.recuperacao.ultima = None;
    }

    fn escrever_meta(&self) {
        let (_, json) = minhas();
        let meta = serde_json::json!({
            "pid": std::process::id(),
            "nome": self.nome_documento(),
            "origem": self.caminho.as_ref().or(self.origem.as_ref()).map(|p| p.to_string_lossy().to_string()),
            "pulso": agora_s(),
        });
        let _ = std::fs::write(json, meta.to_string());
    }

    /// Chamado a cada pulso de 1 s: grava a cópia quando é hora, renova o
    /// pulso e apaga a cópia quando não há mais o que recuperar.
    pub(super) fn pulso_recuperacao(&mut self) -> Task<Msg> {
        let (fshz, _) = minhas();
        // O "Salvar" desmarca o alterado antes de a gravação terminar: até ela
        // acabar, a cópia é o único lugar com as mudanças.
        if self.salvando { return Task::none(); }
        if !self.alterado {
            if self.recuperacao.versoes.is_some() || fshz.exists() {
                self.descartar_recuperacao();
            }
            return Task::none();
        }
        if self.recuperacao.pulso.is_none_or(|p| p.elapsed() >= PULSO) && fshz.exists() {
            self.recuperacao.pulso = Some(Instant::now());
            self.escrever_meta();
        }
        // O modo de captura (prints) mexe na planilha e sai sem "Sair":
        // não deixa cópia órfã na pasta da pessoa.
        let capturando = !cfg!(test) && std::env::var_os("FEATHERSHEET_CAPTURAR").is_some();
        // Com o salvamento automático cobrindo o documento (.fshz salvo), o
        // próprio arquivo é a cópia: gravar outra só dobraria o trabalho.
        let coberto_pelo_auto_salvar = self.config.auto_salvar && self.caminho.as_ref()
            .is_some_and(|p| p.extension().and_then(|e| e.to_str()) == Some("fshz"));
        if capturando || !self.config.recuperacao_automatica || coberto_pelo_auto_salvar
            || self.recuperacao.gravando || self.salvando || self.carregando
            || self.editor_formula.ativo
            || self.assistente_divisao.as_ref().is_some_and(|e| e.conversao_em_andamento())
            || !self.auto_salvar_desde.is_some_and(|t| t.elapsed() >= OCIOSO)
        {
            return Task::none();
        }
        let versoes: Vec<(u32, u32)> = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i)).map(|a| (a.id, a.versao)).collect();
        if self.recuperacao.versoes.as_ref() == Some(&versoes) {
            return Task::none();
        }
        let celulas = (0..self.workbook.len()).filter_map(|i| self.workbook.aba(i))
            .map(|aba| aba.sheet.len() as u64).fold(0u64, u64::saturating_add);
        if feathersheet_io::backup_policy::grande_demais_para_copia_automatica(celulas) {
            return Task::none();
        }
        let intervalo = feathersheet_io::backup_policy::intervalo_autosave(INTERVALO_MS, self.recuperacao.custo_ms, celulas, false);
        if self.recuperacao.ultima.is_some_and(|t| t.elapsed() < Duration::from_millis(intervalo)) {
            return Task::none();
        }
        if std::fs::create_dir_all(pasta()).is_err() {
            return Task::none();
        }
        self.recuperacao.gravando = true;
        self.recuperacao.ultima = Some(Instant::now());
        self.recuperacao.versoes = Some(versoes);
        self.escrever_meta();
        self.recuperacao.pulso = Some(Instant::now());
        let pasta_wb = feathersheet_io::da_workbook::da_workbook(&self.workbook, self.folha as u32);
        Task::perform(async move {
            let inicio = Instant::now();
            tokio::task::spawn_blocking(move || {
                feathersheet_io::arquivo::gravar_atomico_com(&fshz, |arquivo| {
                    feathersheet_io::fshz_write::gravar_fshz_para(arquivo, &pasta_wb).map_err(std::io::Error::other)
                }).map_err(|e| e.to_string())
            }).await.unwrap_or_else(|e| Err(e.to_string()))
                .map(|()| inicio.elapsed().as_millis().min(u128::from(u64::MAX)) as u64)
        }, |r| Msg::Recuperacao(Evento::Gravada(r)))
    }

    pub(super) fn evento_recuperacao(&mut self, evento: Evento) -> Task<Msg> {
        match evento {
            Evento::Gravada(r) => {
                self.recuperacao.gravando = false;
                match r {
                    Ok(ms) => {
                        self.recuperacao.custo_ms = ms;
                        // Salvou enquanto a cópia gravava: ela já não serve.
                        if !self.alterado {
                            self.descartar_recuperacao();
                        }
                    }
                    Err(e) => {
                        self.recuperacao.versoes = None;
                        self.monitor.registrar(Nivel::Aviso, tr!("Recuperação"), &trf!("Cópia de recuperação falhou: {e}"));
                    }
                }
                Task::none()
            }
            Evento::Recuperar(i) => {
                let Some(o) = self.recuperacao.orfas.get(i).cloned() else { return Task::none() };
                // Vai para Temporarios/: abre como planilha nova não salva.
                self.recuperacao.origem_pendente = Some(o.origem.clone().or_else(|| Some(PathBuf::from(&o.nome))));
                self.recuperacao.orfas.remove(i);
                // A cópia passa a esta sessão (que grava a própria logo depois).
                let destino = pasta().join(format!("recuperada-{}.fshz", std::process::id()));
                if std::fs::rename(&o.fshz, &destino).is_err() {
                    self.status = tr!("Não foi possível abrir a cópia recuperada.").into();
                    return Task::none();
                }
                let _ = std::fs::remove_file(&o.json);
                self.recente_destino = Some(destino);
                self.proteger(Pendente::Recente)
            }
            Evento::Descartar(i) => {
                if i < self.recuperacao.orfas.len() {
                    let o = self.recuperacao.orfas.remove(i);
                    apagar(&o.fshz, &o.json);
                }
                Task::none()
            }
        }
    }

    /// Cartão da tela inicial com as planilhas que dá para recuperar.
    pub(super) fn cartao_recuperacao(&self) -> Option<Element<'_, Msg>> {
        if self.recuperacao.orfas.is_empty() {
            return None;
        }
        let mut lista = iced::widget::column![
            row![
                feather_ui::icones::icone("refresh", 18.0),
                text(tr!("Planilhas para recuperar")).size(15).font(iced::Font { weight: iced::font::Weight::Bold, ..Default::default() }),
            ].spacing(8).align_y(Alignment::Center),
            text(tr!("O FeatherSheet fechou sem salvar estas planilhas. Recupere para continuar de onde parou."))
                .size(12).color(COR_TEXTO_MUTED()),
        ].spacing(8);
        for (i, o) in self.recuperacao.orfas.iter().enumerate() {
            let quando = o.quando.duration_since(SystemTime::UNIX_EPOCH).map_or(0, |d| d.as_secs());
            lista = lista.push(row![
                feather_ui::icones::icone("table", 18.0),
                iced::widget::column![
                    text(o.nome.clone()).size(13),
                    text(trf!("Cópia de {} · {:.1} MB", feathersheet_io::recents::quando(quando * 1000, agora_s() * 1000), o.tamanho as f64 / 1_048_576.0))
                        .size(11).color(COR_TEXTO_MUTED()),
                ].spacing(2).width(Length::Fill),
                button(text(tr!("Descartar")).size(12)).padding([6, 12]).style(estilo_botao_secundario)
                    .on_press(Msg::Recuperacao(Evento::Descartar(i))),
                button(text(tr!("Recuperar")).size(12)).padding([6, 14]).style(estilo_botao_primario)
                    .on_press(Msg::Recuperacao(Evento::Recuperar(i))),
            ].spacing(10).align_y(Alignment::Center));
        }
        Some(container(lista).padding([14, 18]).width(Length::Fill).style(|_| container::Style {
            background: Some(iced::Background::Color(COR_PAINEL_FAIXA())),
            border: iced::Border { color: COR_AZUL_ACENTO(), width: 1.0, radius: 10.0.into() },
            ..Default::default()
        }).into())
    }
}

#[cfg(test)]
mod testes {
    use super::*;

    /// Editar sem salvar grava a cópia (depois da pausa); salvar a apaga.
    #[test]
    fn copia_da_planilha_alterada_e_apagada_ao_salvar() {
        let mut a = App::default();
        a.config.recuperacao_automatica = true;
        let _ = a.update(Msg::Colar("Nunca salva\t42".into()));
        assert!(a.alterado);
        a.auto_salvar_desde = Some(Instant::now() - Duration::from_secs(10));
        let _ = a.pulso_recuperacao();
        assert!(a.recuperacao.gravando, "começou a cópia");
        let (fshz, json) = minhas();
        let meta: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&json).unwrap()).unwrap();
        assert_eq!(meta["pid"], std::process::id());
        // A gravação em segundo plano (o Task) termina: aqui, à mão.
        std::fs::write(&fshz, b"copia").unwrap();
        let _ = a.evento_recuperacao(Evento::Gravada(Ok(5)));
        assert!(!a.recuperacao.gravando);
        // Sem mudança nova, não grava de novo.
        a.recuperacao.ultima = None;
        let _ = a.pulso_recuperacao();
        assert!(!a.recuperacao.gravando);
        // Salvou (ou descartou): a cópia some.
        a.alterado = false;
        let _ = a.pulso_recuperacao();
        assert!(!fshz.exists() && !json.exists());
    }

    #[test]
    fn orfa_so_quando_o_pulso_parou_e_nao_e_desta_sessao() {
        let dir = pasta();
        std::fs::create_dir_all(&dir).unwrap();
        let morta = dir.join("999999991.json");
        let viva = dir.join("999999992.json");
        std::fs::write(dir.join("999999991.fshz"), b"x").unwrap();
        std::fs::write(dir.join("999999992.fshz"), b"x").unwrap();
        std::fs::write(&morta, serde_json::json!({"pid": 999999991u64, "nome": "Orçamento.xlsx", "pulso": agora_s() - 600}).to_string()).unwrap();
        std::fs::write(&viva, serde_json::json!({"pid": 999999992u64, "nome": "Viva.fshz", "pulso": agora_s()}).to_string()).unwrap();
        let orfas = procurar_orfas();
        assert!(orfas.iter().any(|o| o.nome == "Orçamento.xlsx"));
        assert!(!orfas.iter().any(|o| o.nome == "Viva.fshz"), "outra janela aberta não é órfã");
        for o in ["999999991", "999999992"] {
            apagar(&dir.join(format!("{o}.fshz")), &dir.join(format!("{o}.json")));
        }
    }
}
