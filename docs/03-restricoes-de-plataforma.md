# Restrições de plataforma

O que o macOS impõe a um app que lê os arquivos do usuário e recomenda o que fazer com eles
(o app nunca apaga). Decidir isso cedo evita
reescrever o app depois.

---

## Full Disk Access (obrigatório)

Sem FDA, o app não lê `~/Library/Caches`, `~/Library/Containers`, `~/Library/Application Support`
nem `~/Library/Mobile Documents`. São exatamente os alvos.

- Concessão é manual: Ajustes > Privacidade e Segurança > Acesso Total ao Disco.
- Não existe API para solicitar via prompt. O app precisa detectar a ausência e instruir.
- Detecção: tentar ler `~/Library/Application Support/com.apple.TCC/TCC.db`. Falha = sem FDA.
- Abrir o painel direto: `x-apple.systempreferences:com.apple.preference.security?Privacy_AllFiles`

O onboarding tem que explicar por que precisa. Pedir FDA sem justificar afasta usuário —
com razão, é a permissão mais poderosa do sistema.

---

## App Sandbox: incompatível

Sandbox restringe o app ao próprio container. Ler `~/Library` inteiro exige sair dele.

Sandbox e FDA são mutuamente excludentes na prática. **Escolher: sem sandbox.**

Consequência direta: **não dá para distribuir pela Mac App Store.** A App Store exige
sandbox sem exceção para esta categoria.

Distribuição: `.dmg` no GitHub Releases.

---

## Notarização

Sem notarizar, o Gatekeeper bloqueia o app em qualquer Mac que não seja o que compilou.
O usuário vê "não pode ser aberto porque o desenvolvedor não pôde ser verificado".

Requer conta Apple Developer — **USD 99/ano**.

Alternativas sem pagar:

1. Usuário compila localmente (`swift build`). Elimina o problema, restringe o público
   a quem tem Xcode.
2. Usuário contorna: botão direito > Abrir, ou Ajustes > Privacidade > "Abrir Mesmo Assim".
   Funciona, mas é fricção e ensina hábito ruim.
3. Distribuir só o CLI via Homebrew tap. Binários de linha de comando sofrem menos com
   Gatekeeper e um tap não custa nada.

**Recomendação: começar pelo CLI via tap.** Adiar os USD 99 até existir demanda pelo app
gráfico.

---

## SIP (System Integrity Protection)

Bloqueia escrita em `/System`, `/usr` (exceto `/usr/local`), `/bin`, `/sbin`.

Não é obstáculo — é rede de proteção. O app não deveria tocar nessas áreas de qualquer
forma. Não instrua o usuário a desativar SIP em nenhuma hipótese.

---

## Recomendar o Lixo, nunca `rm`

O app não apaga nem move nada. Quando a recomendação é remover, o passo dado ao usuário é
mover para o Lixo (no Finder ou no app dono), o equivalente a:

```swift
try FileManager.default.trashItem(at: url, resultingItemURL: nil)
```

- Reversível pelo usuário.
- Respeita permissões e cotas.
- Não requer privilégio elevado.

`rm -rf` em código de limpeza é como esse tipo de app destrói dados. Não recomendar.

Ressalva a comunicar na UI: mover para o Lixo **não libera espaço** até esvaziar. A
recomendação termina com esvaziar o Lixo como passo final explícito.

---

## Operações que exigem root

| Operação | Por quê | Recomendar ao usuário? |
|---|---|---|
| Apagar `/private/var/vm/sleepimage` | Pertence a root | Não. Regenera na próxima suspensão |
| Limpar `/private/var/db` | Sistema | Não. Risco alto |
| `tmutil deletelocalsnapshots` | Depende do sistema | Sim, mas via ferramenta oficial |

O app não executa nenhuma delas. Um helper privilegiado, se existir, só mede (ler como root);
nunca apaga, nem o `sleepimage` (2 GB que voltam sozinhos).

---

## Decisões consolidadas

| Questão | Decisão |
|---|---|
| Sandbox | Não |
| App Store | Não (consequência do acima) |
| Distribuição | GitHub Releases (`.dmg`) + Homebrew tap para o CLI |
| Notarização | Adiar. CLI primeiro |
| Helper privilegiado | Só para medir; nunca apaga |
| Deleção | Nunca pelo app; só recomenda ao usuário |
| Mínimo do sistema | macOS 13 (`MenuBarExtra` do SwiftUI) |
