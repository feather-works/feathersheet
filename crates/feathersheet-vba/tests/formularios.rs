//! #67: o código do formulário (UserForm) convertido junto com o desenho.
use feathersheet_vba::formulario::{ControleVba, FormularioVba, TipoVba};
use feathersheet_vba::{gerar::converter, ModuloVba, ProjetoVba, TipoDeModulo};

fn controle(nome: &str, tipo: TipoVba) -> ControleVba {
    ControleVba { nome: nome.into(), tipo, ..Default::default() }
}

fn projeto() -> ProjetoVba {
    let modulo = |nome: &str, tipo, codigo: &str| ModuloVba { nome: nome.into(), tipo, codigo: codigo.into() };
    ProjetoVba {
        modulos: vec![
            modulo("Modulo1", TipoDeModulo::Padrao, "Sub Abrir()\n    UserForm1.Show\nEnd Sub\n"),
            modulo("UserForm1", TipoDeModulo::Classe, r#"Private Sub UserForm_Initialize()
    ComboBox1.AddItem "Sul"
    Me.ComboBox1.AddItem "Norte"
    Me.Caption = "Cadastro"
    TextBox1.Value = "0"
    MsgBox "oi"
End Sub

Private Sub CommandButton1_Click()
    Range("A1").Value = TextBox1.Value
    Range("B1").Value = Me.ComboBox1.Text
    Me.TextBox1.Text = ""
    Unload Me
End Sub

Private Sub TextBox1_Change()
    Label1.Caption = TextBox1
    CommandButton1.Enabled = TextBox1 <> ""
End Sub

Private Sub UserForm_QueryClose(Cancel As Integer, CloseMode As Integer)
End Sub
"#),
        ],
        formularios: vec![FormularioVba {
            nome: "UserForm1".into(),
            titulo: "UserForm1".into(),
            controles: vec![
                controle("TextBox1", TipoVba::Texto),
                controle("ComboBox1", TipoVba::Lista),
                controle("CommandButton1", TipoVba::Botao),
                controle("Label1", TipoVba::Rotulo),
            ],
            ..Default::default()
        }],
        ..Default::default()
    }
}

#[test]
fn show_campos_unload_e_eventos_dos_controles() {
    let c = converter(&projeto(), "Cadastro");
    let abrir = c.macros.iter().find(|m| m.nome == "Abrir").expect("macro Abrir");
    assert!(abrir.codigo.contains(r#"sheet.showComponent("UserForm1");"#), "{}", abrir.codigo);

    let modulo = c.macros.iter().find(|m| m.nome == "UserForm1").expect("módulo do formulário");
    let js = &modulo.codigo;
    for trecho in [
        r#"sheet.field("TextBox1").value"#,
        r#"sheet.field("ComboBox1").value"#,
        r#"sheet.field("TextBox1").value = "";"#,
        "sheet.hideComponent();",
        r#"sheet.field("Label1").caption = sheet.field("TextBox1").value;"#,
        r#"sheet.field("CommandButton1").enabled = "#,
    ] {
        assert!(js.contains(trecho), "falta {trecho} em\n{js}");
    }
    assert!(!js.contains("// VBA não convertido ("), "{js}");

    let f = &c.formularios[0];
    assert_eq!(f.macros, vec![("CommandButton1".to_string(), "CommandButton1_Click".to_string()), ("TextBox1".to_string(), "TextBox1_Change".to_string())]);
    assert_eq!(f.desenho.titulo, "Cadastro");
    let combo = f.desenho.controles.iter().find(|x| x.nome == "ComboBox1").unwrap();
    assert_eq!(combo.itens, vec!["Sul", "Norte"]);
    assert_eq!(f.desenho.controles[0].valor, "0");
    // O MsgBox do Initialize não roda ao abrir: fica como pendência, com a linha.
    assert!(c.pendencias.iter().any(|p| p.modulo == "UserForm1" && p.vba.contains("MsgBox") && !p.aviso), "{:?}", c.pendencias);
    assert!(c.pendencias.iter().any(|p| p.vba.contains("UserForm_QueryClose") && p.aviso));
}
