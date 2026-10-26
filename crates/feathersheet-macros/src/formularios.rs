//! Formulários das macros (o UserForm do VBA): `sheet.showComponent("nome")`
//! abre o formulário numa janela; um botão roda a macro dele com o que foi
//! preenchido em `valores` (campo → valor). Guardados na pasta
//! (`Workbook::formularios`, `fm` no `.fshz`), como as macros.
use serde::{Deserialize, Serialize};

/// Tetos para um arquivo estranho não travar a janela.
pub const MAX_FORMULARIOS: usize = 64;
pub const MAX_CONTROLES: usize = 200;
/// Imagem guardada em base64 (o `Picture` do VBA): até ~1,5 MB de arquivo.
pub const MAX_IMAGEM: usize = 2_000_000;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoControle {
    #[default]
    Rotulo,
    Texto,
    Numero,
    Senha,
    Caixa,
    Opcao,
    Lista,
    Botao,
    /// Lista aberta de várias linhas (o `ListBox` do VBA); com `multipla`, o valor é a lista das marcadas.
    Listagem,
    /// Escolher um intervalo da planilha (o `RefEdit` do VBA): o valor é o endereço, ex. `Dados!A1:B5`.
    Intervalo,
    /// Moldura com título que agrupa controles (o `Frame` do VBA); no designer, arrastar leva junto o que está dentro.
    Moldura,
    /// Figura (o `Image` do VBA), guardada em `imagem`.
    Imagem,
}

impl TipoControle {
    pub const TODOS: [TipoControle; 12] = [Self::Rotulo, Self::Texto, Self::Numero, Self::Senha, Self::Caixa, Self::Opcao, Self::Lista,
        Self::Listagem, Self::Intervalo, Self::Botao, Self::Moldura, Self::Imagem];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Rotulo => "rotulo", Self::Texto => "texto", Self::Numero => "numero", Self::Senha => "senha",
            Self::Caixa => "caixa", Self::Opcao => "opcao", Self::Lista => "lista", Self::Botao => "botao", Self::Intervalo => "intervalo", Self::Listagem => "listagem",
            Self::Moldura => "moldura", Self::Imagem => "imagem",
        }
    }

    /// Entra em `valores` (rótulo e botão não guardam nada).
    pub fn tem_valor(self) -> bool {
        !matches!(self, Self::Rotulo | Self::Botao | Self::Moldura | Self::Imagem)
    }
}

/// Um controle, com posição e tamanho em px dentro do formulário.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Controle {
    pub tipo: TipoControle,
    /// A chave em `valores`; nas opções, o grupo (todas do mesmo grupo têm o mesmo nome).
    pub nome: String,
    /// O texto mostrado: rótulo, texto do botão, da caixa ou da opção.
    #[serde(default)]
    pub texto: String,
    pub x: f32,
    pub y: f32,
    pub largura: f32,
    pub altura: f32,
    /// Itens da lista suspensa.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub opcoes: Vec<String>,
    /// Lista: aceita texto digitado além dos itens (o `ComboBox` do VBA).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub livre: bool,
    /// Listagem: marca vários itens (`MultiSelect`); o valor vira uma lista.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multipla: bool,
    /// Lista e listagem: itens lidos destas células ao abrir (o `RowSource` do VBA), ex. `Dados!A2:A20`,
    /// um nome ou `=INDIRETO(…)`; no lugar de `opcoes`.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fonte: String,
    /// Valor inicial ("1" marca a caixa ou a opção).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub valor: String,
    /// A macro que roda com `valores`: no botão, ao clicar; nos campos, quando o valor muda
    /// (texto e número: ao confirmar com Enter), como o `Change`/`AfterUpdate` do VBA.
    #[serde(default, rename = "macro", skip_serializing_if = "Option::is_none")]
    pub macro_nome: Option<String>,
    /// Botão: fecha o formulário (depois de rodar a macro, se houver).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fechar: bool,
    /// Botão: limpa os campos depois da macro (cadastro em série).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub limpar: bool,
    /// Botão acionado pelo Enter (o `Default` do VBA).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub padrao: bool,
    /// Botão acionado pelo Esc (o `Cancel` do VBA).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub cancelar: bool,
    /// `Enabled = False`: aparece, mas não aceita mudança.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub desativado: bool,
    /// `Visible = False`.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub oculto: bool,
    /// Aparência; vazio segue o tema. Cores em `#rrggbb`: fundo (`BackColor`), texto (`ForeColor`) e borda.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fundo: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub cor: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub borda: String,
    /// Cantos arredondados, em px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub raio: Option<f32>,
    /// Tamanho da letra, em px.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub letra: Option<f32>,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub negrito: bool,
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub italico: bool,
    /// Família da letra: "serifa" ou "mono" (vazio = a do app).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub familia: String,
    /// Alinhamento do texto: "centro" ou "direita" (vazio = esquerda; botão = centro).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub alinhar: String,
    /// Campo de texto: limite de caracteres (`MaxLength`; 0 = sem limite).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub max: u32,
    /// Campo de texto de várias linhas (`MultiLine`).
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub multilinha: bool,
    /// Balão ao parar o mouse em cima (`ControlTipText`).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub dica: String,
    /// Imagem em base64 (PNG ou JPEG) do controle Imagem.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub imagem: String,
}

fn is_zero(v: &u32) -> bool { *v == 0 }

/// `#rgb`/`#rrggbb` (o `#` é opcional) ou vazio.
pub fn cor_valida(c: &str) -> bool {
    let h = c.trim().trim_start_matches('#');
    c.trim().is_empty() || (matches!(h.len(), 3 | 6) && h.chars().all(|x| x.is_ascii_hexdigit()))
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Formulario {
    pub nome: String,
    #[serde(default)]
    pub titulo: String,
    #[serde(default = "largura_padrao")]
    pub largura: f32,
    #[serde(default = "altura_padrao")]
    pub altura: f32,
    #[serde(default)]
    pub controles: Vec<Controle>,
    /// Cor de fundo do formulário (`BackColor`; vazio = tema).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub fundo: String,
}

fn largura_padrao() -> f32 { 360.0 }
fn altura_padrao() -> f32 { 260.0 }

impl Formulario {
    pub fn novo(nome: &str) -> Self {
        Self { nome: nome.into(), titulo: nome.into(), largura: largura_padrao(), altura: altura_padrao(), controles: Vec::new(), fundo: String::new() }
    }

    /// Os valores iniciais de cada campo (o que `valores` traz sem ninguém mexer).
    pub fn valores_iniciais(&self) -> serde_json::Map<String, serde_json::Value> {
        let mut v = serde_json::Map::new();
        for c in self.controles.iter().filter(|c| c.tipo.tem_valor()) {
            match c.tipo {
                TipoControle::Caixa => { v.insert(c.nome.clone(), (c.valor == "1").into()); }
                // Opção: o grupo vale o texto da marcada ("" se nenhuma).
                TipoControle::Opcao => {
                    let atual = v.entry(c.nome.clone()).or_insert_with(|| "".into());
                    if c.valor == "1" { *atual = c.texto.clone().into(); }
                }
                // Várias escolhas: "a;b" no valor inicial.
                TipoControle::Listagem if c.multipla => {
                    let marcadas: Vec<serde_json::Value> = c.valor.split(';').map(str::trim).filter(|t| !t.is_empty()).map(Into::into).collect();
                    v.insert(c.nome.clone(), marcadas.into());
                }
                TipoControle::Numero => {
                    let n = c.valor.trim().replace(',', ".").parse::<f64>().ok();
                    v.insert(c.nome.clone(), n.map_or(serde_json::Value::Null, Into::into));
                }
                _ => { v.insert(c.nome.clone(), c.valor.clone().into()); }
            }
        }
        v
    }

    fn conferir(&self) -> Result<(), String> {
        let nome = self.nome.trim();
        if nome.is_empty() { return Err("Formulário sem nome.".into()); }
        if !cor_valida(&self.fundo) { return Err(format!("Cor de fundo \"{}\" inválida no formulário \"{nome}\".", self.fundo)); }
        if self.controles.len() > MAX_CONTROLES { return Err(format!("O formulário \"{nome}\" passa de {MAX_CONTROLES} controles.")); }
        if !(80.0..=2000.0).contains(&self.largura) || !(60.0..=2000.0).contains(&self.altura) {
            return Err(format!("O formulário \"{nome}\" tem tamanho fora de 80–2000 × 60–2000 px."));
        }
        for c in &self.controles {
            if c.tipo.tem_valor() && c.nome.trim().is_empty() {
                return Err(format!("Um controle \"{}\" do formulário \"{nome}\" está sem nome.", c.tipo.nome()));
            }
            if ![c.x, c.y, c.largura, c.altura].iter().all(|v| v.is_finite() && *v >= 0.0) {
                return Err(format!("O controle \"{}\" do formulário \"{nome}\" tem posição inválida.", c.nome));
            }
            if let Some(cor) = [&c.fundo, &c.cor, &c.borda].into_iter().find(|x| !cor_valida(x)) {
                return Err(format!("Cor \"{cor}\" inválida no formulário \"{nome}\": use #rrggbb."));
            }
            if c.raio.is_some_and(|r| !(0.0..=200.0).contains(&r)) || c.letra.is_some_and(|l| !(6.0..=96.0).contains(&l)) {
                return Err(format!("O controle \"{}\" do formulário \"{nome}\" tem cantos (0–200) ou letra (6–96) fora do limite.", c.nome));
            }
            if c.imagem.len() > MAX_IMAGEM { return Err(format!("A imagem do formulário \"{nome}\" passa de 1,5 MB.")); }
        }
        // Mesmo nome só entre opções do mesmo grupo.
        let mut vistos = std::collections::BTreeMap::new();
        for c in self.controles.iter().filter(|c| c.tipo.tem_valor()) {
            if let Some(tipo) = vistos.insert(c.nome.trim(), c.tipo) {
                if !(tipo == TipoControle::Opcao && c.tipo == TipoControle::Opcao) {
                    return Err(format!("Dois controles com o nome \"{}\" no formulário \"{nome}\".", c.nome));
                }
            }
        }
        Ok(())
    }
}

/// Os formulários guardados na pasta (`None` = nenhum).
pub fn ler(json: Option<&str>) -> Result<Vec<Formulario>, String> {
    let Some(texto) = json else { return Ok(Vec::new()) };
    let lista: Vec<Formulario> = serde_json::from_str(texto).map_err(|e| format!("Formulários inválidos: {e}"))?;
    conferir(&lista)?;
    Ok(lista)
}

/// O texto para `Workbook::formularios` (`None` se a lista está vazia).
pub fn gravar(lista: &[Formulario]) -> Result<Option<String>, String> {
    conferir(lista)?;
    Ok((!lista.is_empty()).then(|| serde_json::to_string(lista).expect("formulários em memória")))
}

fn conferir(lista: &[Formulario]) -> Result<(), String> {
    if lista.len() > MAX_FORMULARIOS { return Err(format!("Mais de {MAX_FORMULARIOS} formulários na pasta.")); }
    let mut nomes = std::collections::BTreeSet::new();
    for f in lista {
        f.conferir()?;
        if !nomes.insert(f.nome.trim().to_lowercase()) { return Err(format!("Dois formulários com o nome \"{}\".", f.nome)); }
    }
    Ok(())
}

pub fn obter<'a>(lista: &'a [Formulario], nome: &str) -> Option<&'a Formulario> {
    lista.iter().find(|f| f.nome.trim().eq_ignore_ascii_case(nome.trim()))
}

#[cfg(test)]
mod testes {
    use super::*;

    fn controle(tipo: TipoControle, nome: &str, texto: &str, valor: &str) -> Controle {
        Controle { tipo, nome: nome.into(), texto: texto.into(), largura: 100.0, altura: 24.0, valor: valor.into(), ..Default::default() }
    }

    #[test]
    fn valores_iniciais_por_tipo() {
        let mut f = Formulario::novo("cadastro");
        f.controles = vec![
            controle(TipoControle::Rotulo, "", "Nome", ""),
            controle(TipoControle::Texto, "nome", "", "Ana"),
            controle(TipoControle::Numero, "idade", "", "30,5"),
            controle(TipoControle::Caixa, "ativo", "Ativo", "1"),
            controle(TipoControle::Opcao, "plano", "Básico", ""),
            controle(TipoControle::Opcao, "plano", "Pro", "1"),
            controle(TipoControle::Botao, "", "OK", ""),
        ];
        let v = serde_json::Value::Object(f.valores_iniciais());
        assert_eq!(v, serde_json::json!({ "nome": "Ana", "idade": 30.5, "ativo": true, "plano": "Pro" }));
        let texto = gravar(&[f.clone()]).unwrap().unwrap();
        assert_eq!(ler(Some(&texto)).unwrap(), vec![f]);
    }

    #[test]
    fn recusa_nome_repetido_e_controle_sem_nome() {
        let mut f = Formulario::novo("x");
        f.controles = vec![controle(TipoControle::Texto, "a", "", ""), controle(TipoControle::Caixa, "a", "", "")];
        assert!(gravar(&[f.clone()]).unwrap_err().contains("Dois controles"));
        f.controles = vec![controle(TipoControle::Texto, " ", "", "")];
        assert!(gravar(&[f.clone()]).unwrap_err().contains("sem nome"));
        f.controles = vec![Controle { fundo: "#12345".into(), ..controle(TipoControle::Rotulo, "", "", "") }];
        assert!(gravar(&[f.clone()]).unwrap_err().contains("Cor"));
        f.controles[0].fundo = "#abc".into();
        assert!(gravar(&[f]).is_ok());
        assert!(gravar(&[Formulario::novo("a"), Formulario::novo("A")]).unwrap_err().contains("Dois formulários"));
    }
}
