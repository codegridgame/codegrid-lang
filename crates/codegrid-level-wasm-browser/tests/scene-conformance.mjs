// Transport-only driver; scene rules and execution remain in shared Rust.
export function runSceneConformance(request, cases, assert) {
  const call = (operation, fields = {}) => {
    const response = JSON.parse(request(JSON.stringify({api_version: 2, operation, ...fields})));
    assert(response.api_version === 2, 'API-2 response identity');
    return response;
  };
  const results = [];
  for (const fixture of cases) {
    const level = call('load_level', {level_json: fixture.level_json});
    assert(level.status === 'ok', fixture.id + ': load');
    const program = call('compile_program', {source: fixture.source});
    assert(program.status === 'ok', fixture.id + ': compile');
    const started = call('start_evaluation', {level: level.handle, program: program.handle,
      mode: fixture.mode, boundary_mode: fixture.boundary, shuffle_seed: fixture.seed,
      custom_execution_limit: fixture.custom_limit});
    assert(started.status === 'ok', fixture.id + ': start');
    let response;
    for (let i = 0; i < 10000; i++) {
      response = call('advance_evaluation', {evaluation: started.handle, work_budget: fixture.work_budget ?? '1000000'});
      if (response.status === 'result') break;
      assert(response.status === 'pending', fixture.id + ': bounded progress');
    }
    assert(response.status === 'result', fixture.id + ': completion');
    assert(response.result.status === fixture.expected_status, fixture.id + ': terminal status');
    const events = [];
    if (fixture.mode === 'Debug') {
      let cursor = '0';
      for (let i = 0; i < 100; i++) {
        const page = call('scene_feedback', {evaluation_handle: started.handle, after_sequence: cursor, max_events: '1024'});
        assert(page.status === 'ok', fixture.id + ': feedback');
        events.push(...page.events);
        cursor = page.next_sequence;
        if (!page.has_more) {
          const empty = call('scene_feedback', {evaluation_handle: started.handle, after_sequence: cursor, max_events: '1024'});
          assert(empty.events.length === 0, fixture.id + ': acknowledgement');
          break;
        }
      }
    } else {
      assert(call('scene_feedback', {evaluation_handle: started.handle, after_sequence: '0', max_events: '1'}).error.code === 'level_api.invalid_configuration', fixture.id + ': Official privacy');
    }
    results.push({id: fixture.id, result: response.result, events});
    assert(JSON.stringify(call('evaluation_result', {evaluation: started.handle}).result) === JSON.stringify(response.result), fixture.id + ': stable result');
    for (const [kind, handle] of [['evaluation', started.handle], ['program', program.handle], ['level', level.handle]]) {
      assert(call('release', {kind, handle}).status === 'ok', fixture.id + ': release');
    }
  }
  return results;
}
