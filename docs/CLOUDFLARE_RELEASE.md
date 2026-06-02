# Cloudflare 下载源发布说明

打包后的安装包可以放到 Cloudflare 给用户下载。建议文档站继续使用 Cloudflare Pages，安装包和 `latest.json` 使用 Cloudflare R2。

## 推荐目录结构

```text
https://downloads.yuciyuan.top/codex-manager/latest.json
https://downloads.yuciyuan.top/codex-manager/releases/v1.0.15/Codex%20Manager_1.0.15_x64_en-US.msi
```

`latest.json` 会指向当前最新版本的安装包，应用更新检查可以读取这个地址，避免依赖 GitHub。

## Cloudflare 准备

1. 创建一个 R2 bucket，例如 `codex-manager-releases`。
2. 给 R2 bucket 绑定公开自定义域名，例如 `downloads.yuciyuan.top`。
3. 创建 Cloudflare API Token，至少需要当前账号的 R2 object 读写权限。

## GitHub Secrets

在 GitHub 仓库的 `Settings -> Secrets and variables -> Actions` 中添加：

```text
CLOUDFLARE_API_TOKEN=你的 Cloudflare API Token
CLOUDFLARE_ACCOUNT_ID=你的 Cloudflare Account ID
CLOUDFLARE_R2_BUCKET=codex-manager-releases
CLOUDFLARE_DOWNLOAD_BASE_URL=https://downloads.yuciyuan.top
```

可选 GitHub variable：

```text
CLOUDFLARE_DOWNLOAD_PATH_PREFIX=codex-manager
```

如果不设置，默认就是 `codex-manager`。

## 发布流程

推送 tag 后会自动构建、上传 GitHub Release，并在 Cloudflare 配置完整时同步到 R2：

```powershell
git tag v1.0.15
git push origin v1.0.15
```

构建完成后检查：

```text
https://downloads.yuciyuan.top/codex-manager/latest.json
```

## 应用内更新地址

当前应用设置页支持填写自定义 `latest.json` 地址，可以填：

```text
https://downloads.yuciyuan.top/codex-manager/latest.json
```

如果希望所有新安装用户默认走 Cloudflare，需要把 `crates/codex-manager-core/src/update.rs` 里的 `DEFAULT_LATEST_JSON_URL` 改成上面的地址，然后重新发版。
