export function runLevelSmoke(request, fixture, assert) {
 const call = (operation,fields={}) => {const response=JSON.parse(request(JSON.stringify({api_version:1,operation,...fields})));assert(response.api_version===1,'API identity');return response;};
 assert(call('capabilities').capabilities.evaluation_types.join(',')==='ExactIO','capabilities');
 assert(JSON.parse(request('{"api_version":2,"operation":"capabilities"}')).error.code==='level_api.unsupported_version','API version');
 assert(JSON.parse(request('{')).error.code==='level_api.invalid_request','malformed request');
 const level=call('load_level',{level_json:fixture.level}).handle;
 const program=call('compile_program',{source:fixture.source}).handle;
 assert(typeof level==='string'&&typeof program==='string','opaque handles');
 const evaluation=call('start_evaluation',{level,program,mode:'Official',boundary_mode:'Exit',shuffle_seed:'18446744073709551615',custom_execution_limit:'1000'}).handle;
 let response;for(let i=0;i<1000;i++){response=call('advance_evaluation',{evaluation,work_budget:'1000000'});if(response.status==='result')break;assert(response.status==='pending','pending progression');}
 assert(response.status==='result','evaluation completes');assert(response.result.status==='Passed','ExactIO pass');
 assert(response.result.configuration.shuffle_seed==='18446744073709551615','exact wide root seed');
 assert(JSON.stringify(call('evaluation_result',{evaluation}).result)===JSON.stringify(response.result),'stable result');
 assert(call('release',{kind:'evaluation',handle:evaluation}).status==='ok','release evaluation');
 assert(call('evaluation_result',{evaluation}).error.code==='level_api.invalid_handle','stale handle');
 call('release',{kind:'program',handle:program});call('release',{kind:'level',handle:level});
 assert(call('shutdown').status==='ok','shutdown');assert(call('capabilities').error.code==='level_api.shutdown','permanent shutdown');
 return response.result;
}

export function runConformance(request,cases,assert) {
 const call=(operation,fields={})=>JSON.parse(request(JSON.stringify({api_version:1,operation,...fields})));
 const results=[];
 for(const fixture of cases){
  const level=call('load_level',{level_json:fixture.level_json});assert(level.status==='ok',fixture.id+': load');
  const program=call('compile_program',{source:fixture.source});assert(program.status==='ok',fixture.id+': compile');
  const started=call('start_evaluation',{level:level.handle,program:program.handle,mode:fixture.mode,boundary_mode:fixture.boundary,shuffle_seed:fixture.seed,custom_execution_limit:fixture.custom_limit});assert(started.status==='ok',fixture.id+': start');
  let response;for(let i=0;i<10000;i++){response=call('advance_evaluation',{evaluation:started.handle,work_budget:'1000000'});if(response.status==='result')break;assert(response.status==='pending',fixture.id+': advance');}
  assert(response.status==='result',fixture.id+': completion');assert(response.result.status===fixture.expected_status,fixture.id+': expected '+fixture.expected_status+' got '+response.result.status);
  results.push({id:fixture.id,result:response.result});
  call('release',{kind:'evaluation',handle:started.handle});call('release',{kind:'program',handle:program.handle});call('release',{kind:'level',handle:level.handle});
 }
 return results;
}
