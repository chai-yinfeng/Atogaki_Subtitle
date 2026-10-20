# 月夜／来信材质资产

日期：2026-10-19。用户批准深化两套现有主题，其他作品组等待人工审查后再决定。

使用内置 image_gen（imagegen skill 的默认模式）生成，未使用 CLI、真实 Key 或官网原图作为输入；属于原创生成素材，不声称为官方作品或手绘原作。原始 PNG 保留完整输出，未通过脚本重绘、裁切或压缩。

| 文件（相对 ui/src/assets） | 尺寸 | 用途 |
| --- | --- | --- |
| moonlight-painted.png | 1536 × 1024 | 月夜场景与选择卡片；笔触、画布与冷暖光 |
| letter-stationery.png | 1536 × 1024 | 来信场景与选择卡片；纸纤维、折痕与光影 |
| reading-paper.png | 1254 × 1254 | 低强度阅读表面肌理；CSS 叠加各主题色洗 |

原始输出位于本机会话 generated_images 目录；生产引用全部指向仓库资产。三张 PNG 合计约 8.7 MiB；静态加载、不循环播放动画、不添加大面积背景模糊。实际重复边缘仍需随长页面观察，不宣称生成纹理数学无缝。

## 最终提示词

### moonlight-painted.png

Use case: stylized-concept. Asset type: production background illustration for an offline audio-reading desktop app, not a UI mockup. Generate one original wide landscape image 1536x1024 or wider. A quiet nocturnal woodland seen from a sheltered studio opening, midnight petrol blue and deep desaturated teal, loose irregular foliage framing LEFT and RIGHT edges, a small diffused amber moonlight source high on the RIGHT, small warm ochre reflections through cool mist. The central 65% should be low contrast, breathable blue-grey shadow, suitable behind readable panels; detailed painterly texture visible around edges and top. Medium: physically convincing gouache and thin oil paint on toothy linen, dry brush scumbling, broken pigment edges, subtle canvas weave, translucent brush layering, asymmetrical natural composition. Material and tactile painted marks are essential, NOT flat vector silhouettes, NOT smooth digital gradients, NOT glossy fantasy concept art, NOT heavy uniform noise. Calm, literary, restrained. No people, no animals, no buildings, no letters, no text, no logos, no UI, no borders. Original composition; do not replicate any album artwork.

### letter-stationery.png

Use case: stylized-concept. Asset type: original production background for a literary audio-reading app. One wide landscape image 1536x1024. Tactile stationery still life viewed directly from overhead, softly lit by late afternoon window light. Warm neutral unbleached linen paper desk surface, natural irregular paper fibres, slightly cool ivory papers, one partially opened unmarked kraft envelope tucked at far LEFT edge, a narrow stack of blank cream correspondence pages along far RIGHT edge, a few faded vermilion postal-box rule accents at upper right but NO characters. Central 70% is quiet nearly blank finely textured ivory paper, some soft broad light and very subtle crease, suitable behind UI. Medium: editorial still-life photograph with subtle analog print grain, authentic dry cotton fibres, softly deckled paper edges, shallow real occlusion shadows, gently worn folds, no artificial grunge. Restrained Japanese literary book-design sensibility. Uncoated matte stock, NOT glossy, NOT plastic, NOT vector graphic, NOT flat geometric mockup. Neutral warm whites not heavy yellow/sepia. No writing, no text, no logos, no postage stamps, no UI, no people, no pens, no flowers. Original composition, do not reproduce existing album art.

### reading-paper.png

Use case: photorealistic-natural. Asset type: subtle repeatable reading-surface material texture for desktop UI. Square 1024x1024 close-up scan of beautiful uncoated fine cotton-rag drawing paper, NEUTRAL near-white light grey, faint irregular interwoven fibres and tiny natural paper grain. Even diffuse illumination. Extremely subtle low contrast, suitable beneath small dark text. No objects, no edges, no stains, no creases, no writing, no large flecks, no shadows, no pattern, no vignette. Organic physical paper tooth, not random digital noise. Surface fills entire frame edge to edge, visually seamless edges.

## 分层边界

场景在视口中固定；外围采用低透明度色洗，导航采用稳定的半透明底；面板使用带主题色的静态纸面；输入、波形和字幕保持稳定底色，不直接透出复杂场景。来信用页边线和轻微纸张错层，月夜用暖金边缘和冷色阅读表面。系统减少透明度偏好提供不透明降级。

主题沿用原有 ID 和 localStorage，不更改任务配置、字幕样式或导出。背景丢失时保留纯色地面，文字底色与边线也有明确颜色。
