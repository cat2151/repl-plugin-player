// @text uapmd_samples_comment
{
    const samples = repl.sourceRoot() + '/../uapmd/samples';
    const uapmdz = include('builtin/lib/uapmdz.js');
    uapmdz.find(samples).map(path => {
        try {
            repl.project.load(path);
            return uapmdz.describe(path);
        } catch (error) {
            return path + '\n  ' + error.message;
        }
    }).join('\n\n');
}
