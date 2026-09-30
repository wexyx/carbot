// The OpenCode model picker needs to say two different things per model: whether it
// is free, and what it costs when it is not. A missing price is never "free", so a
// catalog that only listed identifiers must stay visibly unknown rather than look
// like a free model.

// Free means every published tier costs zero; anything else needs an account the
// operator supplies through environment variables.
export function modelPrice(model) {
  if (!model?.priced) return {free: false, text: '价格未知', needsAccount: true}
  if (model.free) return {free: true, text: '免费', needsAccount: false}
  if (typeof model.input !== 'number' || typeof model.output !== 'number')
    return {free: false, text: '价格未知', needsAccount: true}
  return {free: false, text: `$${trim(model.input)} / $${trim(model.output)} 每百万 token`, needsAccount: true}
}
function trim(value) {
  return Number.isInteger(value) ? String(value) : String(Number(value.toFixed(2)))
}
export function modelLabel(model) {
  const {free, text} = modelPrice(model)
  const context = Number.isFinite(model?.context) && model.context > 0
    ? ` · ${Math.round(model.context / 1000)}K 上下文`
    : ''
  const tools = model?.tools === false ? ' · 不支持工具调用' : ''
  const status = model?.status && model.status !== 'active' ? ` · ${model.status}` : ''
  return `${model?.id ?? '未知模型'} · ${text}${context}${tools}${status}`
}
// Free first, then cheapest, matching the order the CLI wizard prints. The backend
// already ranks the catalog; sorting again keeps the list usable if it ever changes.
export function orderModels(models) {
  return [...(Array.isArray(models) ? models : [])].sort((a, b) => {
    const left = modelPrice(a), right = modelPrice(b)
    return Number(right.free) - Number(left.free)
      || (a?.input ?? Infinity) - (b?.input ?? Infinity)
      || String(a?.id ?? '').localeCompare(String(b?.id ?? ''))
  })
}
// The backend answers either with a priced catalog or, when that fails, with bare
// identifiers. The picker must tell those apart rather than implying a price.
export function readCatalog(payload) {
  const models = orderModels(payload?.models)
  return {
    models,
    // An empty catalog has nothing to price, so it is never "priced" either.
    priced: payload?.priced === true && models.length > 0 && models.every(model => model.priced === true),
    signedIn: payload?.signed_in === true,
    reason: models.length ? '' : payload?.signed_in === true
      ? 'OpenCode 未返回任何模型'
      : '未检测到已登录的 OpenCode，请先运行 opencode auth login，或手动填写模型 ID',
  }
}
