// The SVG mark and editable RML share these paths. Runtime artwork contains no scripts.
import { writeFileSync } from 'node:fs';
import { dirname, join } from 'node:path';
import { fileURLToPath } from 'node:url';
const root = dirname(fileURLToPath(import.meta.url));
const ink = '14122B', paper = 'FBF7F0', pink = 'E5197F';
const v = (x, y, incoming, outgoing) => ({ x, y, incoming, outgoing });
const body = [v(-58,-112,[-78,-112]),v(32,-112),v(92,-52),v(92,78,undefined,[92,97]),v(67,104,[86,104]),v(-66,104,undefined,[-87,104]),v(-92,78,[-92,98]),v(-88,-80,undefined,[-87,-102])];
const fold = [v(0,0),v(60,60),v(19,60,undefined,[5,60]),v(0,41,[0,55]),v(0,0)];
const underside = [v(32,-58,undefined,[45,-45]),v(92,-29,[83,-40]),v(92,-52),v(51,-52,undefined,[40,-52])];
const mouth = [v(-17,-6,[-19,-10],[-9,-2]),v(17,-6,[9,-2],[19,-10]),v(13,10,[17,6],[6,19]),v(-12,10,[-4,19],[-16,5])];
const tongue = [v(-9,9,undefined,[-3,2]),v(10,9,[4,2],[6,16]),v(-7,12,[-2,18])];
const f = n => Number(n.toFixed(5));
function pathSVG(points) {
  let path = `M ${points[0].x} ${points[0].y}`;
  for (let i=1;i<=points.length;i++) {
    const a=points[i-1], b=points[i%points.length];
    if (a.outgoing || b.incoming) {
      const c=a.outgoing??[a.x,a.y], d=b.incoming??[b.x,b.y];
      path+=` C ${c.join(' ')} ${d.join(' ')} ${b.x} ${b.y}`;
    } else path+=` L ${b.x} ${b.y}`;
  }
  return path+' Z';
}
function pathRML(points) {
  return '<PointsPath isClosed="true">'+points.map(p=>{
    if (!p.incoming&&!p.outgoing) return `<StraightVertex x="${p.x}" y="${p.y}"/>`;
    const attrs=[];
    for (const [label,handle] of [['in',p.incoming],['out',p.outgoing]]) if(handle){
      attrs.push(`${label}Rotation="${f(Math.atan2(handle[1]-p.y,handle[0]-p.x))}"`,`${label}Distance="${f(Math.hypot(handle[0]-p.x,handle[1]-p.y))}"`);
    }
    return `<CubicDetachedVertex x="${p.x}" y="${p.y}" ${attrs.join(' ')}/>`;
  }).join('')+'</PointsPath>';
}
const fill = color => `<Fill><SolidColor colorValue="FF${color}"/></Fill>`;
const stroke = `<Stroke thickness="11" join="round" cap="round"><SolidColor colorValue="FF${ink}"/></Stroke>`;
const shape = (name,points,color,outline=false) => `<Shape name="${name}">${pathRML(points)}${outline?stroke:''}${fill(color)}</Shape>`;
const ellipse = (name,x,y,w,h,color) => `<Shape name="${name}" x="${x}" y="${y}"><Ellipse width="${w}" height="${h}"/>${fill(color)}</Shape>`;
function svgMark(flick=false) {
  const path=(p,color,outline=false)=>`<path d="${pathSVG(p)}" fill="#${color}"${outline?` stroke="#${ink}" stroke-width="11" stroke-linejoin="round"`:''}/>`;
  const eye=(x,y,w,h)=>`<g transform="translate(${x} ${y}) rotate(12)"><ellipse rx="${w/2}" ry="${h/2}" fill="#${ink}"/><ellipse cx="-2" cy="-7" rx="2.5" ry="5" fill="#${paper}"/></g>`;
  return `<svg xmlns="http://www.w3.org/2000/svg" viewBox="88 72 224 248"><g transform="translate(200 205)">${path(body,paper,true)}${path(underside,pink)}<g transform="translate(32 -112)${flick?' rotate(-18)':''}">${path(fold,pink,true)}</g>${eye(-33,15,18,36)}${flick?`<path d="M17 24 Q26 12 35 24" stroke="#${ink}" stroke-width="6" stroke-linecap="round" fill="none"/>`:eye(26,25,15,30)}<g transform="translate(-2 56)">${path(mouth,ink)}${path(tongue,pink)}</g></g></svg>\n`;
}
writeFileSync(join(root,'mark.svg'),svgMark());
writeFileSync(join(root,'hello.svg'),svgMark(true));
function keys(id,key,frames,hold=false) {
  return `<KeyedObject objectId="0:${id}"><KeyedProperty propertyKey="${key}">`+frames.map(([frame,value])=>`<KeyFrameDouble frame="${frame}" value="${value}" interpolationType="${hold?'hold':'cubic'}">${hold?'':'<CubicEaseInterpolator x1="0.22" y1="1" x2="0.36" y2="1"/>'}</KeyFrameDouble>`).join('')+'</KeyedProperty></KeyedObject>';
}
const condition = `<TransitionViewModelCondition><TransitionPropertyViewModelComparator><BindablePropertyBoolean><DataBindContext sourcePathIds="0:40-0:42" propertyKey="634"/></BindablePropertyBoolean></TransitionPropertyViewModelComparator><TransitionValueBooleanComparator value="true"/></TransitionViewModelCondition>`;
const intro = [
  keys(15,14,[[0,234],[9,310],[14,292],[20,304],[24,304]]),
  keys(15,16,[[0,0.3],[7,1],[9,1.3],[14,.85],[20,1],[24,0]]),
  keys(15,17,[[0,.3],[7,1.15],[9,.68],[14,1.15],[20,1],[24,0]]),
  keys(15,18,[[0,1],[20,1],[24,0]]),
  keys(11,16,[[0,0],[14,0],[16,.74],[22,1.06],[31,.98],[40,1],[48,1]]),
  keys(11,17,[[0,0],[14,0],[16,.12],[22,1.06],[31,.97],[40,1],[48,1]]),
  keys(11,14,[[0,310],[16,310],[22,287],[31,313],[40,310],[48,310]]),
  keys(11,15,[[0,-.10],[16,-.10],[24,.04],[33,-.02],[48,0]]),
  keys(13,15,[[0,0],[26,0],[32,-.45],[38,.08],[45,0],[48,0]]),
  keys(14,17,[[0,1],[33,1],[36,.12],[39,.12],[43,1],[48,1]]),
  keys(81,243,[[0,0],[9,1],[32,2]],true),
].join('\n');
const idle = keys(11,17,[[0,1],[90,1.005],[180,1]])+keys(14,17,[[0,1],[139,1],[143,.08],[148,1],[180,1]])+keys(81,243,[[0,2]],true);
const exit = keys(10,18,[[0,1],[12,0]])+keys(11,16,[[0,1],[12,.92]])+keys(11,17,[[0,1],[12,.92]])+keys(81,243,[[0,3],[12,4]],true);
const rml=`<Rive version="1" kind="fragment">
<Artboard name="Corner Hello" id="0:2" width="400" height="400" styleId="0:5" defaultStateMachineId="0:7" viewModelId="0:40" viewModelInstanceId="0:41">
<LayoutComponentStyle name="Artboard Style" id="0:5"/>
<CustomPropertyGroup name="Launch cues" x="-240" y="0"><CustomPropertyNumber name="cue" id="0:81"><DataBindContext sourcePathIds="0:40-0:43" propertyKey="243" direction="true"/></CustomPropertyNumber></CustomPropertyGroup>
<Node name="Greeting" id="0:10">
  <Node name="Character" id="0:11" x="200" y="310" scaleX="0" scaleY="0">
    <Node name="Paper" id="0:12" y="-105">
      <Node name="Face">
        <Node name="Right eye" id="0:14" x="26" y="25" rotation="0.20944">${ellipse('Eye glint',-2,-7,5,10,paper)}${ellipse('Ink eye',0,0,15,30,ink)}</Node>
        <Node name="Left eye" x="-33" y="15" rotation="0.20944">${ellipse('Eye glint',-2,-7,5,10,paper)}${ellipse('Ink eye',0,0,18,36,ink)}</Node>
        <Node name="Smile" x="-2" y="56">${shape('Pink tongue',tongue,pink)}${shape('Smile opening',mouth,ink)}</Node>
      </Node>
      <Node name="Flexible corner" id="0:13" x="32" y="-112">${shape('Magenta fold',fold,pink,true)}</Node>
      ${shape('Fold underside',underside,pink)}
      ${shape('Paper silhouette',body,paper,true)}
    </Node>
  </Node>
  <Node name="Pink dot" id="0:15" x="200" y="234">${ellipse('Dot',0,0,30,30,pink)}</Node>
</Node>
<StateMachine name="Launch" id="0:7"><StateMachineLayer name="Greeting">
  <AnyState x="0" y="-160"/><ExitState x="660" y="-160"/>
  <EntryState x="0" y="0"><StateTransition stateToId="0:20"/></EntryState>
  <AnimationState id="0:20" x="180" y="0" animationId="0:60" reset="true"><StateTransition stateToId="0:21" duration="0" enableExitTime="true" exitTimeIsPercetange="true" exitTime="100"/></AnimationState>
  <AnimationState id="0:21" x="380" y="0" animationId="0:61" reset="true"><StateTransition stateToId="0:22" duration="0">${condition}</StateTransition></AnimationState>
  <AnimationState id="0:22" x="580" y="0" animationId="0:62" reset="true"/>
</StateMachineLayer></StateMachine>
<LinearAnimation name="Hello" id="0:60" duration="48" fps="60">${intro}</LinearAnimation>
<LinearAnimation name="Quiet hold" id="0:61" duration="180" fps="60" loopValue="1">${idle}</LinearAnimation>
<LinearAnimation name="Into the app" id="0:62" duration="12" fps="60">${exit}</LinearAnimation>
</Artboard>
<ViewModel name="Launch" id="0:40" defaultInstanceId="0:41"><ViewModelPropertyBoolean name="ready" id="0:42"/><ViewModelPropertyNumber name="cue" id="0:43"/><ViewModelInstance name="Default" id="0:41" exports="true"><ViewModelInstanceBoolean viewModelPropertyId="0:42" propertyValue="false"/><ViewModelInstanceNumber viewModelPropertyId="0:43" propertyValue="0"/></ViewModelInstance></ViewModel>
</Rive>\n`;
writeFileSync(join(root,'scene.rml'),rml);
writeFileSync(join(root,'rive.yaml'),'name: corner-hello\nmain: Corner Hello\nlogs:\n  file: build/rive.log\n  problems: build/problems.log\n');
console.log('Wrote matching SVG marks and script-free RML.');
