//! Preferências locais da interface nativa, fora do documento.
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Configuracao {
    /// Regionalização de fórmulas, números e separadores de texto.
    pub idioma: String,
    #[serde(default = "csv_detectar_padrao")]
    pub csv_detectar_separador: bool,
    pub escuro: bool,
    /// Quando ativo, segue a preferência clara/escura do sistema na abertura.
    pub tema_auto: bool,
    pub acento: usize,
    /// None = preferência legada; Some(false) restaura a logo de fábrica
    /// sem alterar a cor de destaque dos controles do aplicativo.
    pub marca_personalizada: Option<bool>,
    pub barra_propria: bool,
    pub faixa_recolhida: bool,
    /// Aba fixa ativa da Ribbon, salva como identificador estável (independente do idioma).
    /// A aba contextual "Tabela" nunca substitui esta preferência.
    pub faixa_aba: String,
    pub linhas_visiveis: u32,
    pub colunas_visiveis: u32,
    pub zoom: u16,
    /// Faixa permitida para o zoom da grade (não altera o documento).
    pub zoom_minimo: u16,
    pub zoom_maximo: u16,
    /// Fonte e tamanho de fallback, usados só quando a célula não tem estilo próprio.
    pub fonte_grade: String,
    pub tamanho_fonte_grade: u16,
    /// Destaque visual da linha/coluna ativa — preferência por usuário, não por pasta.
    pub realce_eixos: bool,
    /// Preferências de interface da planilha (não fazem parte do FSHZ).
    pub linhas_grade: bool,
    pub barra_formulas: bool,
    pub mostrar_formulas: bool,
    /// Fórmulas → Opções de cálculo: manual espera o F9 (como no Excel,
    /// preferência da pessoa e não do arquivo).
    pub calculo_manual: bool,
    /// Fórmulas → Usadas recentemente, a mais nova primeiro.
    pub funcoes_recentes: Vec<String>,
    /// Dados → Fontes recentes do OpenQuery (caminho ou URL), a mais nova primeiro.
    pub fontes_recentes: Vec<String>,
    /// Gravação periódica apenas de documentos .fshz já salvos.
    pub auto_salvar: bool,
    pub intervalo_auto_salvar: u16,
    /// Cópia de recuperação de toda planilha alterada (inclusive nunca
    /// salva), para voltar depois de queda de energia ou travamento.
    pub recuperacao_automatica: bool,
    /// Limite da vigia RSS na abertura em GB: 0 desliga, 2 a 128 ativa.
    pub limite_memoria_gb: u16,
    /// Threads para abrir arquivo e calcular em paralelo: 1 até o total − 1
    /// (uma fica para a janela). 0 = metade do processador, o padrão.
    pub nucleos: u16,
    /// Atalhos editados pelo usuário: id estável → combinação canônica.
    pub atalhos: std::collections::BTreeMap<String, String>,
    /// Aba Desenhar: ler a pressão da mesa (XInput2 numa janela X11/XWayland,
    /// WM_POINTER no Windows). Vale na próxima abertura.
    pub caneta_pressao: bool,
    /// Linux/Wayland: onde abrir a janela — `auto` (XWayland se houver mesa,
    /// para a caneta clicar), `x11` ou `wayland`. Vale na próxima abertura.
    pub janela_linux: String,
    /// Fuso de AGORA()/HOJE(), em minutos a leste de UTC; `None` = o do sistema.
    pub fuso_minutos: Option<i32>,
    /// Curva de resposta da pressão (gama) aplicada ao capturar.
    pub caneta_gama: f32,
    /// Sem mesa: espessura pela velocidade (rápido = mais fino).
    pub caneta_velocidade: bool,
    /// Indicador "pressão 0,63" durante o traço (modo de teste).
    pub caneta_mostrar_pressao: bool,
    /// A ponta da caneta na planilha liga o modo desenhar sozinha (o mouse
    /// ou o Esc voltam à seleção), como o "Desenhar com caneta
    /// automaticamente" do Excel.
    pub caneta_auto_desenhar: bool,
    /// "Seus dados" (Primeiros passos): autor dos comentários e das
    /// propriedades do arquivo. Vazio usa o nome do sistema.
    pub autor_nome: String,
    pub autor_iniciais: String,
    pub autor_organizacao: String,
    /// A janela Primeiros passos já foi mostrada (abre sozinha só uma vez).
    pub primeiros_passos_vistos: bool,
    /// Última versão cujas novidades a pessoa viu; diferente da atual abre
    /// "O que há de novo" sozinha.
    pub versao_vista: String,
    /// O que aparece na barra de status (chaves de `barra_status::ITENS`),
    /// escolhido pelo botão direito no rodapé.
    pub barra_status: Vec<String>,
    /// MCP ao vivo: uma IA desta máquina pode usar a planilha aberta.
    /// Desligado de fábrica (privacidade por padrão).
    pub mcp_ativo: bool,
    /// Com o MCP ligado, a IA só lê (não escreve, não formata, não salva).
    pub mcp_so_leitura: bool,
    /// O arquivo de preferências já existia ao abrir (não é instalação nova).
    /// Não é gravado.
    #[serde(skip)]
    pub existia: bool,
}
fn csv_detectar_padrao() -> bool { true }

/// O idioma da interface para o idioma do sistema ("pt-PT", "en_GB.UTF-8"…):
/// português vira pt-BR, espanhol es-ES; qualquer outro, inglês.
pub fn idioma_suportado(sistema: Option<&str>) -> &'static str {
    match sistema.map(|s| s.to_ascii_lowercase()) {
        Some(s) if s.starts_with("pt") => "pt-BR",
        Some(s) if s.starts_with("es") => "es-ES",
        Some(_) => "en-US",
        None => "pt-BR",
    }
}

/// Barra de status numa instalação nova: o padrão do Excel (média,
/// contagem, soma) mais a posição, o aviso de erro e o zoom.
pub fn barra_status_padrao() -> Vec<String> {
    ["celula", "media", "contagem", "soma", "aviso_erro", "zoom"]
        .iter().map(|s| (*s).to_string()).collect()
}

/// Curvas da pressão oferecidas nas Configurações (rótulo, gama).
pub const CURVAS_CANETA: [(&str, f32); 4] =
    [("Leve (0,5)", 0.5), ("Média (0,75)", 0.75), ("Linear (1,0)", 1.0), ("Firme (1,5)", 1.5)];
/// Onde abrir a janela no Linux/Wayland (rótulo, valor gravado).
pub const JANELAS_LINUX: [(&str, &str); 3] = [
    ("Automático (X11 se houver mesa)", "auto"),
    ("Sempre X11 (XWayland)", "x11"),
    ("Sempre Wayland", "wayland"),
];
pub const ROTULOS_JANELAS_LINUX: [&str; 3] = [JANELAS_LINUX[0].0, JANELAS_LINUX[1].0, JANELAS_LINUX[2].0];
/// Só os rótulos de [`CURVAS_CANETA`] (a lista suspensa pede `&'static [&str]`).
pub const ROTULOS_CURVAS_CANETA: [&str; 4] =
    [CURVAS_CANETA[0].0, CURVAS_CANETA[1].0, CURVAS_CANETA[2].0, CURVAS_CANETA[3].0];

impl Default for Configuracao {
    fn default() -> Self {
        Self {
            idioma: "pt-BR".into(), csv_detectar_separador: true, escuro: true, tema_auto: false, acento: 0, marca_personalizada: None, barra_propria: true,
            faixa_recolhida: false, faixa_aba: "inicio".into(), linhas_visiveis: 28, colunas_visiveis: 12, zoom: 100, zoom_minimo: 50, zoom_maximo: 200,
            fonte_grade: "sans".into(), tamanho_fonte_grade: 13,
            realce_eixos: true, linhas_grade: true, barra_formulas: true,
            mostrar_formulas: false, calculo_manual: false, funcoes_recentes: Vec::new(), fontes_recentes: Vec::new(), auto_salvar: false, intervalo_auto_salvar: 15, recuperacao_automatica: true,
            limite_memoria_gb: 6,
            nucleos: 0,
            atalhos: std::collections::BTreeMap::new(),
            caneta_pressao: true,
            janela_linux: "auto".into(),
            fuso_minutos: None,
            caneta_gama: CURVAS_CANETA[1].1,
            caneta_velocidade: true,
            caneta_mostrar_pressao: false,
            caneta_auto_desenhar: true,
            autor_nome: String::new(),
            autor_iniciais: String::new(),
            autor_organizacao: String::new(),
            primeiros_passos_vistos: false,
            versao_vista: String::new(),
            barra_status: barra_status_padrao(),
            mcp_ativo: false,
            mcp_so_leitura: false,
            existia: false,
        }
    }
}
fn caminho() -> Option<PathBuf> {
    std::env::var_os("XDG_CONFIG_HOME").map(PathBuf::from)
        .or_else(|| std::env::var_os("HOME").map(|h| PathBuf::from(h).join(".config")))
        // No Windows, HOME pode não existir. APPDATA mantém a preferência
        // fora dos arquivos da planilha e persiste entre sessões.
        .or_else(|| {
            #[cfg(windows)]
            { std::env::var_os("APPDATA").map(PathBuf::from) }
            #[cfg(not(windows))]
            { None }
        })
        .map(|p| p.join("feathersheet/config-iced.json"))
}
impl Configuracao {
    pub const ZOOM_ABSOLUTO_MIN: u16 = 25;
    pub const ZOOM_ABSOLUTO_MAX: u16 = 400;

    pub fn normalizar_zoom(&mut self) {
        self.zoom_minimo = self.zoom_minimo.clamp(Self::ZOOM_ABSOLUTO_MIN, Self::ZOOM_ABSOLUTO_MAX);
        self.zoom_maximo = self.zoom_maximo.clamp(self.zoom_minimo, Self::ZOOM_ABSOLUTO_MAX);
        self.zoom = self.zoom.clamp(self.zoom_minimo, self.zoom_maximo);
    }

    pub fn zoom_limitado(&self, percentual: u16) -> u16 {
        percentual.clamp(self.zoom_minimo, self.zoom_maximo)
    }

    /// Se os limites cruzarem, o extremo oposto acompanha a preferência.
    pub fn definir_limite_zoom(&mut self, minimo: bool, percentual: u16) {
        let valor = percentual.clamp(Self::ZOOM_ABSOLUTO_MIN, Self::ZOOM_ABSOLUTO_MAX);
        if minimo {
            self.zoom_minimo = valor;
            self.zoom_maximo = self.zoom_maximo.max(valor);
        } else {
            self.zoom_maximo = valor;
            self.zoom_minimo = self.zoom_minimo.min(valor);
        }
        self.normalizar_zoom();
    }

    pub fn separador_csv(&self) -> Option<char> {
        if self.csv_detectar_separador { None }
        else { Some(if self.idioma == "en-US" { ',' } else { ';' }) }
    }

    /// Antes desta preferência existir, acento diferente do padrão era escolha manual.
    pub fn logo_usa_acento(&self) -> bool {
        self.marca_personalizada.unwrap_or(self.acento != 0)
    }

    /// Permite testar leitura real em arquivos temporários sem redirecionar
    /// HOME ou XDG_CONFIG_HOME (que são globais entre threads).
    fn carregar_de(path: &std::path::Path) -> Self {
        let lido: Option<Self> = std::fs::read(path).ok()
            .and_then(|b| serde_json::from_slice(&b).ok());
        let existia = lido.is_some();
        let mut c = lido.unwrap_or_else(|| Self {
            // Primeira abertura: o idioma do computador, se a interface o tem.
            idioma: idioma_suportado(sys_locale::get_locale().as_deref()).into(),
            ..Default::default()
        });
        c.existia = existia;
        if !matches!(c.idioma.as_str(), "pt-BR" | "en-US" | "es-ES") { c.idioma = "pt-BR".into(); }
        c.acento = c.acento.min(feather_ui::tema::ACENTOS.len().saturating_sub(1));
        c.linhas_visiveis = c.linhas_visiveis.clamp(10, 50);
        c.colunas_visiveis = c.colunas_visiveis.clamp(6, 24);
        c.normalizar_zoom();
        if !matches!(c.fonte_grade.as_str(), "sans" | "serif" | "mono") {
            c.fonte_grade = "sans".into();
        }
        if ![11, 12, 13, 14, 16].contains(&c.tamanho_fonte_grade) {
            c.tamanho_fonte_grade = 13;
        }
        if ![2, 5, 15, 30, 60].contains(&c.intervalo_auto_salvar) {
            c.intervalo_auto_salvar = 15;
        }
        c.limite_memoria_gb = crate::app::limite_memoria::normalizar_limite(c.limite_memoria_gb as f64);
        if c.nucleos as usize > feathersheet_core::paralelismo::maximo() {
            c.nucleos = 0;
        }
        c.atalhos.retain(|id, valor| !id.is_empty() && valor.len() <= 40);
        c.autor_nome = c.autor_nome.chars().take(80).collect();
        c.autor_iniciais = c.autor_iniciais.chars().take(4).collect();
        c.autor_organizacao = c.autor_organizacao.chars().take(80).collect();
        if !matches!(c.janela_linux.as_str(), "auto" | "x11" | "wayland") {
            c.janela_linux = "auto".into();
        }
        if !CURVAS_CANETA.iter().any(|&(_, g)| g == c.caneta_gama) {
            c.caneta_gama = CURVAS_CANETA[1].1;
        }
        // Preferências legadas podem não ter o campo; valores inválidos ou
        // contextuais jamais devem deixar a interface numa aba inexistente.
        if !matches!(c.faixa_aba.as_str(),
            "inicio" | "inserir" | "dados" | "revisao" | "exibir" | "macros" | "ferramentas")
        {
            c.faixa_aba = "inicio".into();
        }
        c
    }

    fn gravar_em(&self, path: &std::path::Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir)?;
        }
        // Atômico: uma queda no meio deixaria o JSON cortado e todas as preferências voltariam ao padrão.
        feathersheet_io::arquivo::gravar_atomico(path, &serde_json::to_vec_pretty(self).map_err(std::io::Error::other)?)
    }

    pub fn carregar() -> Self {
        caminho().map(|p| Self::carregar_de(&p)).unwrap_or_default()
    }

    // Evita que testes de UI mudem a configuração real do usuário.
    #[cfg(test)]
    pub fn gravar(&self) {}

    #[cfg(not(test))]
    pub fn gravar(&self) {
        if let Some(path) = caminho() { let _ = self.gravar_em(&path); }
    }
}

#[cfg(test)]
mod testes_ui010 {
    use super::*;
    use std::sync::atomic::{AtomicU64, Ordering};
    static CONTADOR: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn preferencias_da_0_0_2_contam_como_atualizacao_e_sem_arquivo_e_instalacao_nova() {
        let arquivo = arquivo_teste();
        assert!(!Configuracao::carregar_de(&arquivo).existia);
        // Arquivo da 0.0.2: sem versao_vista nem primeiros_passos_vistos.
        std::fs::create_dir_all(arquivo.parent().unwrap()).unwrap();
        std::fs::write(&arquivo, r#"{"zoom":125,"acento":5}"#).unwrap();
        let c = Configuracao::carregar_de(&arquivo);
        assert!(c.existia && c.versao_vista.is_empty() && !c.primeiros_passos_vistos);
        // O campo não vai para o arquivo.
        c.gravar_em(&arquivo).unwrap();
        assert!(!std::fs::read_to_string(&arquivo).unwrap().contains("existia"));
    }

    #[test]
    fn primeira_abertura_segue_o_idioma_do_sistema() {
        assert_eq!(idioma_suportado(Some("pt-BR")), "pt-BR");
        assert_eq!(idioma_suportado(Some("pt_PT.UTF-8")), "pt-BR");
        assert_eq!(idioma_suportado(Some("en-GB")), "en-US");
        assert_eq!(idioma_suportado(Some("de-DE")), "en-US");
        assert_eq!(idioma_suportado(Some("es_MX.UTF-8")), "es-ES");
        assert_eq!(idioma_suportado(None), "pt-BR");
    }

    #[test]
    fn ribbon_restaura_aba_fixa_e_ignora_contextual_ou_config_antiga() {
        let arquivo = arquivo_teste();
        let mut config = Configuracao::default();
        config.faixa_aba = "dados".into();
        config.gravar_em(&arquivo).unwrap();
        assert_eq!(Configuracao::carregar_de(&arquivo).faixa_aba, "dados");

        // JSON escrito por versões anteriores continua utilizável.
        std::fs::write(&arquivo, r#"{"zoom":125}"#).unwrap();
        assert_eq!(Configuracao::carregar_de(&arquivo).faixa_aba, "inicio");

        // A aba de contexto some com a seleção; nunca deve ser restaurada.
        std::fs::write(&arquivo, r#"{"faixa_aba":"tabela"}"#).unwrap();
        assert_eq!(Configuracao::carregar_de(&arquivo).faixa_aba, "inicio");

        let _ = std::fs::remove_file(&arquivo);
        if let Some(dir) = arquivo.parent() { let _ = std::fs::remove_dir(dir); }
    }

    fn arquivo_teste() -> PathBuf {
        let i = CONTADOR.fetch_add(1, Ordering::Relaxed);
        std::env::temp_dir().join(format!("feathersheet-ui010-{}-{i}", std::process::id()))
            .join("config-iced.json")
    }

    #[test]
    fn marca_padrao_vs_acento_manual_preserva_preferencias_antigas() {
        let mut c=Configuracao::default();
        assert!(!c.logo_usa_acento());
        c.acento=2; // Preferência legada de uma cor escolhida.
        assert!(c.logo_usa_acento());
        c.marca_personalizada=Some(false);
        assert!(!c.logo_usa_acento()); // Restaurar apenas a logo.
        c.marca_personalizada=Some(true);
        assert!(c.logo_usa_acento()); // Acento explícito, mesmo índice 0.
        c.acento=0;
        assert!(c.logo_usa_acento());
    }
    #[test]
    fn preferencias_de_planilha_persistem_sem_afetar_celulas() {
        let p = arquivo_teste();
        let c = Configuracao {
            idioma: "en-US".into(), csv_detectar_separador: true, escuro: false, tema_auto: true, acento: 4, marca_personalizada: None, barra_propria: false,
            faixa_recolhida: true, linhas_visiveis: 33,
            colunas_visiveis: 18, zoom: 175, zoom_minimo: 75, zoom_maximo: 300, realce_eixos: false,
            fonte_grade: "mono".into(), tamanho_fonte_grade: 14,
            linhas_grade: false, barra_formulas: false, mostrar_formulas: true,
            auto_salvar: true, intervalo_auto_salvar: 5, atalhos: Default::default(),
            ..Default::default()
        };
        c.gravar_em(&p).unwrap();
        let lida = Configuracao::carregar_de(&p);
        assert_eq!(lida.idioma, "en-US");
        assert!(!lida.escuro && lida.tema_auto && !lida.barra_propria);
        assert!(lida.faixa_recolhida);
        assert_eq!(lida.acento, 4);
        assert_eq!((lida.linhas_visiveis, lida.colunas_visiveis, lida.zoom), (33, 18, 175));
        assert_eq!((lida.zoom_minimo, lida.zoom_maximo), (75, 300));
        assert!(!lida.realce_eixos, "o destaque desligado deve sobreviver ao reinício");
        assert!(!lida.linhas_grade && !lida.barra_formulas && lida.mostrar_formulas);
        assert!(lida.auto_salvar);
        assert_eq!(lida.intervalo_auto_salvar, 5);
        assert_eq!(lida.fonte_grade, "mono");
        assert_eq!(lida.tamanho_fonte_grade, 14);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn realce_linha_coluna_ativo_e_desativo_sobrevivem_reabertura() {
        let p = arquivo_teste();
        let mut c = Configuracao::default();
        assert!(c.realce_eixos, "primeiro uso deve manter padrão ligado");
        c.realce_eixos = false;
        c.gravar_em(&p).unwrap();
        assert!(!Configuracao::carregar_de(&p).realce_eixos);
        c.realce_eixos = true;
        c.gravar_em(&p).unwrap();
        assert!(Configuracao::carregar_de(&p).realce_eixos);
        let json = std::fs::read_to_string(&p).unwrap();
        assert!(json.contains("\"realce_eixos\": true"));
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn zoom_configuravel_preserva_configuracoes_legadas_e_valida_limites() {
        let p = arquivo_teste();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, br#"{"zoom":325,"zoom_minimo":125,"zoom_maximo":300}"#).unwrap();
        let c = Configuracao::carregar_de(&p);
        assert_eq!((c.zoom_minimo, c.zoom_maximo, c.zoom), (125, 300, 300));
        std::fs::write(&p, br#"{"zoom":10,"zoom_minimo":450,"zoom_maximo":35}"#).unwrap();
        let c = Configuracao::carregar_de(&p);
        assert_eq!((c.zoom_minimo, c.zoom_maximo, c.zoom), (400, 400, 400));
        std::fs::write(&p, br#"{"zoom":175}"#).unwrap();
        let c = Configuracao::carregar_de(&p);
        assert_eq!((c.zoom_minimo, c.zoom_maximo, c.zoom), (50, 200, 175));
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }

    #[test]
    fn cruzar_limites_move_o_outro_extremo_e_corrige_zoom() {
        let mut c = Configuracao::default();
        c.definir_limite_zoom(true, 250);
        assert_eq!((c.zoom_minimo, c.zoom_maximo, c.zoom), (250, 250, 250));
        c.definir_limite_zoom(false, 75);
        assert_eq!((c.zoom_minimo, c.zoom_maximo, c.zoom), (75, 75, 75));
    }

    #[test]
    fn preferencia_antiga_invalida_e_json_corrompido_nao_travam_a_ui() {
        let p = arquivo_teste();
        std::fs::create_dir_all(p.parent().unwrap()).unwrap();
        std::fs::write(&p, br#"{"zoom":65535,"linhas_visiveis":0,"colunas_visiveis":999,"acento":99}"#).unwrap();
        let v = Configuracao::carregar_de(&p);
        assert_eq!((v.zoom, v.linhas_visiveis, v.colunas_visiveis), (200, 10, 24));
        assert!(v.acento < feather_ui::tema::ACENTOS.len());
        assert!(v.realce_eixos, "preferências antigas sem o campo devem manter destaque padrão");
        assert!(!v.tema_auto, "a configuração antiga deve preservar a escolha manual");
        std::fs::write(&p, b"{").unwrap();
        assert_eq!(Configuracao::carregar_de(&p).zoom, 100);
        std::fs::remove_dir_all(p.parent().unwrap()).unwrap();
    }
}
