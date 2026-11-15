// %% @text name_set_scope_target
// @text scope_set_comment
{
    const scope = include('builtin/lib/scope.js');
    const scopeTrackId = uapmd.sequencer.getTrackInfos()[0].trackId;
    scope.setTarget({trackId:scopeTrackId});
}
