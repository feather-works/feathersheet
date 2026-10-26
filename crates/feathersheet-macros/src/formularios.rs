//! Formulários das macros (o UserForm do VBA): `sheet.showComponent("nome")`
//! abre o formulário numa janela; um botão roda a macro dele com o que foi
//! preenchido em `valores` (campo → valor). Guardados na pasta
//! (`Workbook::formularios`, `fm` no `.fshz`), como as macros.
use serde::{Deserialize, Serialize};

/// Tetos para um arquivo estranho não travar a janela.
pub const MAX_FORMULARIOS: usize = 64;
pub const MAX_CONTROLES: usize = 200;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum TipoControle {
    Rotulo,
    Texto,
    Numero,
    Senha,
    Caixa,
    Opcao,
    Lista,
    Botao,
    /// Escolher um intervalo da planilha (o `RefEdit` do VBA): o valor é o endereço, ex. `Dados!A1:B5`.
    Intervalo,
}

impl TipoControle {
    pub const TODOS: [TipoControle; 9] = [Self::Rotulo, Self::Texto, Self::Numero, Self::Senha, Self::Caixa, Self::Opcao, Self::Lista, Self::Intervalo, Self::Botao];

    pub fn nome(self) -> &'static str {
        match self {
            Self::Rotulo => "rotulo", Self::Texto => "texto", Self::Numero => "numero", Self::Senha => "senha",
            Self::Caixa => "caixa", Self::Opcao => "opcao", Self::Lista => "lista", Self::Botao => "botao", Self::Intervalo => "intervalo",
        }
    }

    /// Entra em `valores` (rótulo e botão não guardam nada).
    pub fn tem_valor(self) -> bool {
        !matches!(self, Self::Rotulo | Self::Botao)
    }
}

/// Um controle, com posição e tamanho em px dentro do formulário.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
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
    /// Lista: itens lidos destas células ao abrir (o `RowSource` do VBA), ex. `Dados!A2:A20`,
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
}

fn largura_padrao() -> f32 { 360.0 }
fn altura_padrao() -> f32 { 260.0 }

impl Formulario {
    pub fn novo(nome: &str) -> Self {
        Self { nome: nome.into(), titulo: nome.into(), largura: largura_padrao(), altura: altura_padrao(), controles: Vec::new() }
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
        Controle { tipo, nome: nome.into(), texto: texto.into(), x: 0.0, y: 0.0, largura: 100.0, altura: 24.0,
            opcoes: Vec::new(), fonte: String::new(), valor: valor.into(), macro_nome: None, fechar: false, limpar: false, padrao: false, cancelar: false, desativado: false, oculto: false }
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
        assert!(gravar(&[f]).unwrap_err().contains("sem nome"));
        assert!(gravar(&[Formulario::novo("a"), Formulario::novo("A")]).unwrap_err().contains("Dois formulários"));
    }
}
