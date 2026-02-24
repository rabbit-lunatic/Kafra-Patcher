# Kafra Patcher

[![Rust](https://github.com/ArturllVale/Kafra-Patcher/actions/workflows/rust.yml/badge.svg)](https://github.com/ArturllVale/Kafra-Patcher/actions/workflows/rust.yml)
[![License: CC BY-NC 4.0](https://img.shields.io/badge/License-CC%20BY--NC%204.0-lightgrey.svg)](LICENSE)
[![Windows](https://img.shields.io/badge/platform-Windows-lightgrey)]()


> Patcher customizável e multiplataforma para clientes Ragnarok Online, baseado no antigo projeto **rpatchur**.

![Screenshot](https://i.imgur.com/mE51Iif.png)

---

## 📋 Índice

- [Funcionalidades](#-funcionalidades)
- [Requisitos](#-requisitos)
- [Instalação Rápida](#-instalação-rápida)
- [Configuração Completa](#-configuração-completa-kpatcheryml)
- [Criando Botões e UI Customizada](#-criando-botões-e-ui-customizada)
- [Janela Sem Bordas e Transparência](#-janela-sem-bordas-e-transparência)
- [Sistema de Atualizações](#-sistema-de-atualizações)
- [Callback Functions (JavaScript)](#-callback-functions-javascript)
- [Compilação do Projeto](#-compilação-do-projeto)
- [Exemplos](#-exemplos)
- [Licença](#-licença)

---

## ✨ Funcionalidades

| Recurso                 | Descrição                                      |
| ----------------------- | ---------------------------------------------- |
| **UI Web Customizável** | Interface feita com HTML/CSS/JS - como um site |
| **Configuração YAML**   | Arquivo externo simples de configurar          |
| **HTTP/HTTPS**          | Suporte a conexões seguras                     |
| **Patches GRF**         | Versões 0x101, 0x102, 0x103 e 0x200            |
| **Formato THOR**        | Compatível com Thor Patcher                    |
| **SSO Login**           | Funciona como launcher com autenticação        |
| **Patches Manuais**     | Permite aplicar patches locais                 |
| **Múltiplos Mirrors**   | Redundância de servidores                      |
| **Janela Customizada**  | Sem bordas, transparente, arredondada          |

---

## 📦 Requisitos

- **Windows 10/11** (ou Linux/macOS)
- **WebView2 Runtime** (incluído no Windows 11, [baixar para Windows 10](https://developer.microsoft.com/microsoft-edge/webview2/))
- Arquivos do cliente Ragnarok Online

---

## 🚀 Instalação Rápida

1. Baixe a [última release](https://github.com/ArturllVale/Kafra-Patcher/releases)
2. Extraia na pasta do seu cliente RO
3. Crie o arquivo `kpatcher.yml` (veja configuração abaixo)
4. Execute `KPatcher.exe`

---

## 🔒 Protegendo sua Configuração (Embed Config)

Por segurança, você pode **embutir** o arquivo `kpatcher.yml` dentro do executável `KPatcher.exe`.

O conteúdo é **comprimido e criptografado (AES-256)**, o que:

1. Oculta as URLs do seu servidor.
2. Evita que usuários editem a configuração.
3. Dificulta a engenharia reversa e extração de dados sensíveis.

O utilitário `mkpatch.exe` possui uma interface gráfica para isso:

1. Abra o `mkpatch.exe` (sem argumentos)
2. Vá na aba **Embed Config**
3. Selecione o seu `KPatcher.exe` original
4. Selecione o seu `kpatcher.yml` configurado
5. Clique em **Embutir Config no EXE**

Um novo arquivo será gerado (ex: `KPatcher_embedded.exe`). Você pode distribuir este arquivo **sem** o `kpatcher.yml` junto.

---

## ⚙️ Configuração Completa (kpatcher.yml)

O arquivo `kpatcher.yml` deve estar na **mesma pasta** do executável. Aqui está uma configuração completa:

```yaml
# ═══════════════════════════════════════════════════════════════
# CONFIGURAÇÃO DA JANELA
# ═══════════════════════════════════════════════════════════════
window:
  title: Meu Servidor RO # Título da janela
  width: 780 # Largura em pixels
  height: 580 # Altura em pixels
  resizable: false # Janela redimensionável?

  # ─── Janela Customizada (Opcional) ───
  frameless: true # Remove bordas e barra de título
  border_radius: 20 # Cantos arredondados (em pixels)

  # ─── Transparência da Janela (Opcional) ───
  # body {
  #           background: transparent;
  #       }

# ═══════════════════════════════════════════════════════════════
# BOTÃO JOGAR
# ═══════════════════════════════════════════════════════════════
play:
  path: ragexe.exe # Executável do jogo
  arguments: ["1sak1"] # Argumentos (opcional)
  exit_on_success: true # Fechar patcher ao iniciar jogo?
  play_with_error: false # Habilitar botão Play se atualização falhar?
  minimize_on_start: false # Minimizar patcher ao iniciar jogo? (requer exit_on_success: false)

# ═══════════════════════════════════════════════════════════════
# BOTÃO CONFIGURAÇÕES
# ═══════════════════════════════════════════════════════════════
setup:
  path: Setup.exe # Executável de setup
  arguments: [] # Argumentos (opcional)
  exit_on_success: false # Fechar patcher ao abrir setup?

# ═══════════════════════════════════════════════════════════════
# CONFIGURAÇÃO WEB E PATCHES
# ═══════════════════════════════════════════════════════════════
web:
  # URL da página HTML (pode ser local ou remota)
  index_url: https://meuservidor.com/patcher/index.html

  # Para testes locais, use:
  # index_url: file:///C:/MeuPatcher/index.html

  preferred_patch_server: Servidor Principal # Servidor prioritário

  patch_servers:
    - name: Servidor Principal
      plist_url: https://meuservidor.com/patcher/plist.txt
      patch_url: https://meuservidor.com/patcher/data/

    - name: Servidor Backup
      plist_url: https://backup.meuservidor.com/plist.txt
      patch_url: https://backup.meuservidor.com/data/

# ═══════════════════════════════════════════════════════════════
# CONFIGURAÇÃO DO GRF
# ═══════════════════════════════════════════════════════════════
client:
  default_grf_name: meuservidor.grf # GRF principal para patches

# ═══════════════════════════════════════════════════════════════
# OPÇÕES DE PATCHING
# ═══════════════════════════════════════════════════════════════
patching:
  in_place: true # Patchear GRF diretamente
  check_integrity: true # Verificar integridade dos downloads
  create_grf: true # Criar GRF se não existir
```

---

## 🎨 Criando Botões e UI Customizada

O Kafra Patcher usa **HTML/CSS/JS** para a interface. Você pode criar qualquer design usando tecnologias web padrão.

### Comandos Disponíveis (external.invoke)

Use `external.invoke('comando')` no seu JavaScript/HTML para interagir com o patcher:

| Comando         | Descrição                | Exemplo                                       |
| --------------- | ------------------------ | --------------------------------------------- |
| `play`          | Inicia o jogo            | `onclick="external.invoke('play')"`           |
| `setup`         | Abre configurações       | `onclick="external.invoke('setup')"`          |
| `exit`          | Fecha o patcher          | `onclick="external.invoke('exit')"`           |
| `minimize`      | Minimiza a janela        | `onclick="external.invoke('minimize')"`       |
| `start_drag`    | Inicia arraste da janela | `onmousedown="external.invoke('start_drag')"` |
| `start_update`  | Inicia atualização       | `onclick="external.invoke('start_update')"`   |
| `cancel_update` | Cancela atualização      | `onclick="external.invoke('cancel_update')"`  |
| `manual_patch`  | Aplica patch manual      | `onclick="external.invoke('manual_patch')"`   |
| `reset_cache`   | Limpa cache              | `onclick="external.invoke('reset_cache')"`    |

### Exemplo: Botões Básicos

```html
<!-- Botão Jogar -->
<button onclick="external.invoke('play')" id="btn-play" disabled>
  🎮 Jogar
</button>

<!-- Botão Sair -->
<button onclick="external.invoke('exit')" id="btn-exit">❌ Sair</button>

<!-- Botão Minimizar -->
<button onclick="external.invoke('minimize')" id="btn-minimize">─</button>

<!-- Botão Configurações -->
<button onclick="external.invoke('setup')" id="btn-setup">
  ⚙️ Configurações
</button>
```

### Exemplo: Menu Dropdown com Ações

```html
<div class="dropdown">
  <button class="dropdown-toggle">Opções</button>
  <div class="dropdown-menu">
    <a href="#" onclick="external.invoke('cancel_update')"
      >❌ Cancelar Atualização</a
    >
    <a href="#" onclick="external.invoke('start_update')"
      >🔄 Reiniciar Atualização</a
    >
    <a href="#" onclick="external.invoke('manual_patch')">📁 Patch Manual</a>
    <a href="#" onclick="external.invoke('reset_cache')">🗑️ Limpar Cache</a>
  </div>
</div>
```

### Exemplo: Abrir URL no Navegador

```html
<button onclick="openUrl('https://meuservidor.com/register')">
  📝 Criar Conta
</button>

<script>
  function openUrl(url) {
    external.invoke(
      JSON.stringify({
        function: "open_url",
        parameters: { url: url },
      }),
    );
  }
</script>
```

### Exemplo: Login SSO (Launcher)

```html
<form onsubmit="startGame(); return false;">
  <input type="text" id="login" placeholder="Usuário" required />
  <input type="password" id="password" placeholder="Senha" required />
  <button type="submit">🔐 Conectar</button>
</form>

<script>
  function startGame() {
    var login = document.getElementById("login").value;
    var password = document.getElementById("password").value;

    external.invoke(
      JSON.stringify({
        function: "login",
        parameters: {
          login: login,
          password: password,
        },
      }),
    );
  }
</script>
```

---

## 🪟 Janela Sem Bordas e Transparência

### Removendo Bordas (Frameless)

Adicione `frameless: true` no seu `kpatcher.yml`:

```yaml
window:
  frameless: true
```

> ⚠️ **Importante**: Com `frameless: true`, você precisa criar seus próprios botões de minimizar/fechar e área de arraste!

### Tornando a Janela Arrastável

Use `start_drag` no `onmousedown` de qualquer elemento que você quer que sirva como barra de título:

```html
<!-- Barra de título arrastável -->
<div class="title-bar" onmousedown="external.invoke('start_drag')">
  <span>Meu Servidor RO</span>
  <button onclick="external.invoke('minimize')">─</button>
  <button onclick="external.invoke('exit')">✕</button>
</div>

<style>
  .title-bar {
    background: #333;
    color: white;
    padding: 10px;
    cursor: move;
    display: flex;
    justify-content: space-between;
    align-items: center;
  }
</style>
```

### Fundo Transparente

Para criar janelas com formatos customizados (não retangulares), use a cor de transparência:

```yaml
window:
  frameless: true
```

Então no seu CSS, use essa cor como fundo:

````css
```css body {
  background: transparent;
  margin: 0;
  padding: 0;
}

.patcher-content {
  background: url("meu-background.png") no-repeat;
  /* OU use um gradiente, cor sólida, etc */
}
````

### Cantos Arredondados

```yaml
window:
  frameless: true
  border_radius: 20 # Raio em pixels
```

### Exemplo Completo: Janela Customizada

```yaml
window:
  title: Meu Servidor
  width: 800
  height: 600
  frameless: true
  border_radius: 15
```

```html
<!DOCTYPE html>
<html>
  <head>
    <style>
      * {
        margin: 0;
        padding: 0;
        box-sizing: border-box;
      }
      body {
        background: transparent;
      }

      .window {
        width: 800px;
        height: 600px;
        background: linear-gradient(135deg, #1a1a2e 0%, #16213e 100%);
        border-radius: 15px;
        overflow: hidden;
      }

      .titlebar {
        height: 40px;
        background: rgba(0, 0, 0, 0.3);
        display: flex;
        align-items: center;
        justify-content: space-between;
        padding: 0 15px;
        cursor: move;
      }

      .titlebar button {
        border: none;
        background: none;
        color: white;
        font-size: 16px;
        cursor: pointer;
        padding: 5px 10px;
      }

      .titlebar button:hover {
        background: rgba(255, 255, 255, 0.1);
      }
      .close-btn:hover {
        background: #e74c3c !important;
      }
    </style>
  </head>

  <body>
    <div class="window">
      <div class="titlebar" onmousedown="external.invoke('start_drag')">
        <span style="color: white;">Meu Servidor RO</span>
        <div>
          <button onclick="external.invoke('minimize')">─</button>
          <button class="close-btn" onclick="external.invoke('exit')">✕</button>
        </div>
      </div>

      <!-- Conteúdo do patcher -->
      <div class="content">
        <!-- ... -->
      </div>
    </div>
  </body>
</html>
```

---

## 🔄 Sistema de Atualizações

### Como Funciona

1. O patcher lê `plist_url` para obter a lista de patches
2. Compara com o cache local para identificar novos patches
3. Baixa os arquivos `.thor`/`.rgz`/`.gpf` de `patch_url`
4. Aplica os patches no GRF definido em `default_grf_name`

### Estrutura no Servidor Web

```
https://meuservidor.com/patcher/
├── index.html          # UI do patcher
├── plist.txt           # Lista de patches
└── data/
    ├── 1.thor          # Patch 1
    ├── 2.thor          # Patch 2
    └── update_jan.thor # Patch de janeiro
```

### Formato do plist.txt

Simples lista de arquivos, um por linha:

```
1.thor
2.thor
3.thor
update_jan.thor
hotfix_001.thor
```

### Formatos de Patch Suportados

| Formato | Descrição                     | Recomendado |
| ------- | ----------------------------- | ----------- |
| `.grf`  | Formato Padrão                | ⭐ Sim      |
| `.thor` | Formato Thor Patcher (legado) | ⭐ Sim      |
| `.rgz`  | GRF comprimido (Gzip)         | ⭐ Sim      |
| `.gpf`  | GRF Patch File                | ⭐ Sim      |

---

## 📞 Callback Functions (JavaScript)

O patcher chama automaticamente estas funções do seu JavaScript para informar o progresso:

### patchingStatusReady()

Chamada quando o jogo está pronto para jogar.

```javascript
function patchingStatusReady() {
  document.getElementById("btn-play").disabled = false;
  document.getElementById("progress-text").textContent = "Pronto!";
  document.getElementById("progress-bar").style.width = "100%";
}
```

### patchingStatusError(errorMsg, playWithError)

Chamada quando ocorre um erro na atualização.

- `errorMsg`: Mensagem de erro
- `playWithError`: Boolean indicando se o botão Play deve ser habilitado (baseado na configuração `play_with_error` do YAML)

```javascript
function patchingStatusError(errorMsg, playWithError) {
  document.getElementById("progress-text").textContent = "Erro: " + errorMsg;
  document.getElementById("progress-bar").classList.add("error");

  // Se configurado, habilita o botão Play mesmo com erro
  if (playWithError) {
    document.getElementById("btn-play").disabled = false;
  }
}
```

### patchingStatusDownloading(nbDownloaded, nbTotal, bytesPerSec)

Chamada durante o download.

```javascript
function patchingStatusDownloading(nbDownloaded, nbTotal, bytesPerSec) {
  var percent = (100 * nbDownloaded) / nbTotal;
  var speed = humanFileSize(bytesPerSec) + "/s";

  document.getElementById("progress-bar").style.width = percent + "%";
  document.getElementById("progress-text").textContent =
    "Baixando: " + nbDownloaded + "/" + nbTotal + " - " + speed;
}

function humanFileSize(bytes) {
  var i = bytes == 0 ? 0 : Math.floor(Math.log(bytes) / Math.log(1024));
  return (
    (bytes / Math.pow(1024, i)).toFixed(2) + " " + ["B", "KB", "MB", "GB"][i]
  );
}
```

### patchingStatusInstalling(nbInstalled, nbTotal)

Chamada durante a instalação dos patches.

```javascript
function patchingStatusInstalling(nbInstalled, nbTotal) {
  var percent = (100 * nbInstalled) / nbTotal;
  document.getElementById("progress-bar").style.width = percent + "%";
  document.getElementById("progress-text").textContent =
    "Instalando: " + nbInstalled + "/" + nbTotal;
}
```

### patchingStatusPatchApplied(fileName)

Chamada quando um patch manual é aplicado.

```javascript
function patchingStatusPatchApplied(fileName) {
  alert("Patch aplicado com sucesso: " + fileName);
}
```

### notificationInProgress()

Chamada quando já existe uma atualização em andamento.

```javascript
function notificationInProgress() {
  alert("Já existe uma atualização em andamento!");
}
```

### mediaPause() / mediaResume()

Chamadas automaticamente quando a janela é minimizada/restaurada. Use para controlar BGM/vídeos.

```javascript
function mediaPause() {
  document.querySelectorAll("audio, video").forEach(function (el) {
    if (!el.paused) {
      el.dataset.wasPlaying = "true";
      el.pause();
    }
  });
}
function mediaResume() {
  document.querySelectorAll("audio, video").forEach(function (el) {
    if (el.dataset.wasPlaying === "true") {
      el.play();
      el.dataset.wasPlaying = "";
    }
  });
}
```

### Exemplo: BGM com Volume Persistente

```html
<audio id="bgm" src="music/theme.mp3" autoplay loop controls></audio>

<script>
  // Volume persistence via localStorage
  var bgm = document.getElementById("bgm");
  var savedVol = localStorage.getItem("bgmVolume");
  if (savedVol !== null) bgm.volume = parseFloat(savedVol);
  bgm.addEventListener("volumechange", function () {
    localStorage.setItem("bgmVolume", bgm.volume);
  });
</script>
```

---

## 🔨 Compilação do Projeto

### Requisitos

- [Rust 1.49+](https://rustup.rs/)
- Git

### Estrutura do Projeto

```
Kafra-Patcher/
├── kpatcher/     # Código do patcher (UI, patching)
├── mkpatch/      # Gerador de patches THOR
├── gruf/         # Biblioteca GRF/THOR
└── examples/     # Exemplos de UI e configuração
```

### Compilação

```bash
# Clonar repositório
git clone https://github.com/ArturllVale/Kafra-Patcher.git
cd Kafra-Patcher

# Compilar release
cargo build --release

# Executáveis estarão em: target/release/
```

### Compilação para Windows 32-bit

```bash
rustup target add i686-pc-windows-msvc
cargo build --target=i686-pc-windows-msvc --release
```

## 📂 Exemplos

A pasta `examples/` contém exemplos prontos para uso:

| Exemplo           | Descrição                                                    |
| ----------------- | ------------------------------------------------------------ |
| `bootstrap/`      | UI completa com Bootstrap, barra de progresso e notificações |
| `basic_launcher/` | Launcher simples com login SSO                               |
| `kpatcher.yml`    | Configuração de exemplo completa                             |
| `patch.yml`       | Configuração de patch de exemplo                             |

Para usar um exemplo:

1. Copie os arquivos para junto do `KPatcher.exe`
2. Edite `kpatcher.yml` com suas URLs
3. Injete o `kpatcher.yml` no `KPatcher.exe` com o `mkpatch.exe`
4. Execute o patcher

---

## 📜 Licença

Copyright (c) 2020-2026 Kafra Patcher developers

- Desenvolvido por: **L1nkZ** / Antigo **rpatchur**
- Mantenedor: **Lumen#0110** / Atual **Kafra Patcher**

Distribuído sob licença [CC BY-NC 4.0](LICENSE).
