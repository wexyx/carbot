// A neutral operation category, not the model's claim that an operation is safe.
export function approvalSummary(item, workspace){
 if(!workspace)return {title:'批准管理操作',description:item.warning||'确认后执行所选管理操作。'}
 if(!item.command)return {title:'访问工作目录外的文件',description:'请求读取或写入指定目录，请核对目标路径。'}
 const command=item.command.trim()
 let title='执行 Shell 命令'
 if(/^(?:\/bin\/)?(?:pwd|ls)(?:\s|$)/.test(command))title='查看目录信息'
 else if(/^(?:python\d*|python3)(?:\s|$)/.test(command))title='运行 Python 脚本'
 else if(/^(?:cargo|npm|pnpm|yarn)\s+(?:test|build|check)(?:\s|$)/.test(command))title='运行测试或构建'
 else if(/^git\s/.test(command))title='执行 Git 操作'
 return {title,description:'命令可能修改文件或联网；此描述不是安全结论，可展开核对完整命令。'}
}
